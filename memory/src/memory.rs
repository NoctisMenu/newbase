use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use windows::core::BOOL;
use windows::core::PCSTR;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingA, VirtualQuery, VirtualQueryEx, MEMORY_BASIC_INFORMATION,
    MEM_COMMIT, MEM_IMAGE, MEM_MAPPED, PAGE_READONLY, PAGE_READWRITE,
};
use windows::Win32::System::ProcessStatus::K32GetMappedFileNameA;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SendMessageA, SetWindowsHookExA,
    ShowWindow, UnhookWindowsHookEx, SW_HIDE, SW_SHOW, WH_SHELL, WM_APPCOMMAND,
};

use crate::types::MemoryError;

const SHARED_MEM_WPARAM_STRUCT: usize = 0x800;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static PROCESS_BASE: AtomicU64 = AtomicU64::new(0);
static LOCAL_SHARED_MEMORY: AtomicU64 = AtomicU64::new(0);
static REMOTE_SHARED_MEMORY: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
struct ExploitState {
    process_id: u32,
    thread_id: u32,
    process_hwnd: HWND,
    base: usize,
    local_shared_memory: usize,
    remote_shared_memory: usize,
    set_protect: usize,
    mr_data_addr_ptr: usize,
    mr_data_size_ptr: usize,
    mr_data_addr_orig: usize,
    mr_data_size_orig: usize,
    set_mrprot: usize,
    t: usize,
    w: usize,
    write_fn: u64,
    nt: HMODULE,
    sh: HMODULE,
    ddll: HMODULE,
}

unsafe impl Send for ExploitState {}
unsafe impl Sync for ExploitState {}

static mut STATE: Option<ExploitState> = None;

#[repr(C)]
struct WParam {
    memcpy: u64,
    arg2: u64,
    function_ptr: u64,
    arg1: u64,
    lock: u64,
    dst_length: u16,
    dst_max_length: u16,
    _pad0: u32,
    dst_buffer: u64,
    src_length: u16,
    src_max_length: u16,
    _pad1: u32,
    src_buffer: u64,
    val: u64,
}

#[repr(C)]
pub struct WParamLayout {
    pub memcpy: u64,
    pub arg2: u64,
    pub function_ptr: u64,
    pub arg1: u64,
    pub lock: u64,
    pub dst_length: u16,
    pub dst_max_length: u16,
    pub _pad0: u32,
    pub dst_buffer: u64,
    pub src_length: u16,
    pub src_max_length: u16,
    pub _pad1: u32,
    pub src_buffer: u64,
    pub val: u64,
}

pub fn pattern_scan_module_public(module: *const u8, signature: &str) -> Option<usize> {
    pattern_scan_module(module, signature)
}

pub fn init_driver_diagnostic(pid: u32) -> Result<(), &'static str> {
    unsafe {
        let hwnd = find_window_by_pid(pid).ok_or("find_window_by_pid failed")?;
        eprintln!("  [ok] hwnd = {:?}", hwnd.0);
        let thread_id = GetWindowThreadProcessId(hwnd, None);
        if thread_id == 0 {
            return Err("GetWindowThreadProcessId returned 0");
        }
        eprintln!("  [ok] thread_id = {}", thread_id);

        let nt = LoadLibraryA(PCSTR(b"ntdll.dll\0".as_ptr())).map_err(|_| "LoadLibrary ntdll")?;
        let sh =
            LoadLibraryA(PCSTR(b"shell32.dll\0".as_ptr())).map_err(|_| "LoadLibrary shell32")?;
        let ddll =
            LoadLibraryA(PCSTR(b"uxtheme.dll\0".as_ptr())).map_err(|_| "LoadLibrary uxtheme")?;
        eprintln!(
            "  [ok] modules loaded: nt={:?} sh={:?} ddll={:?}",
            nt.0, sh.0, ddll.0
        );

        let local_shared = find_shared_memory_local()
            .ok_or("find_shared_memory_local failed - no Discord shared mem in this process")?;
        eprintln!("  [ok] local_shared = {:#x}", local_shared);

        let h_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .map_err(|_| "OpenProcess failed")?;
        eprintln!("  [ok] h_process opened");

        let remote_shared =
            find_shared_memory_remote(h_process, thread_id, hwnd, nt, ddll, local_shared)
                .ok_or("find_shared_memory_remote failed - target has no matching shared mem")?;
        eprintln!("  [ok] remote_shared = {:#x}", remote_shared);

        let base = find_process_base(h_process).ok_or("find_process_base failed")?;
        eprintln!("  [ok] process base = {:#x}", base);

        let _ = CloseHandle(h_process);

        let tramp_pattern =
            "48 83 EC ?? 48 8b da 48 85 d2 74 ?? 83 7a ?? ?? 75 ?? 83 7a ?? ?? 75 ??";
        let mut t = pattern_scan_module(sh.0 as *const u8, tramp_pattern)
            .ok_or("pattern scan T trampoline failed")?;
        t = t & 0xFFFFFFFFFFFFFFF0;
        eprintln!("  [ok] t = {:#x}", t);

        let w = t;
        eprintln!("  [ok] w = t (ROP: trampoline used directly as hook proc)");

        let set_protect_pattern = "48 8b e9 48 8b d1 41 8d 48 03";
        let mut set_protect = pattern_scan_module(nt.0 as *const u8, set_protect_pattern)
            .ok_or("pattern scan set_protect failed")?;
        while *(set_protect as *const u8) != 0xCC {
            set_protect -= 1;
        }
        set_protect += 1;
        eprintln!("  [ok] set_protect = {:#x}", set_protect);

        let mr_data_pattern = "48 8B 05 ?? ?? ?? ?? 4C 8D 44 24 ?? 48 89 44 24";
        let mut set_mrprot = pattern_scan_module(nt.0 as *const u8, mr_data_pattern)
            .ok_or("pattern scan set_mrprot failed")?;
        let disp = *((set_mrprot + 3) as *const i32);
        let mr_data_addr_ptr = set_mrprot + disp as usize + 7;
        let mr_data_size_ptr = mr_data_addr_ptr - 0x10;
        let mr_data_addr_orig = *(mr_data_addr_ptr as *const usize);
        let mr_data_size_orig = *(mr_data_size_ptr as *const usize);
        while *(set_mrprot as *const u8) != 0xCC {
            set_mrprot -= 1;
        }
        set_mrprot += 1;
        eprintln!(
            "  [ok] set_mrprot = {:#x}, mr_data_addr = {:#x}",
            set_mrprot, mr_data_addr_orig
        );

        let write_fn = GetProcAddress(nt, PCSTR(b"RtlCopyString\0".as_ptr()))
            .ok_or("GetProcAddress RtlCopyString failed")? as u64;
        eprintln!("  [ok] write_fn (RtlCopyString) = {:#x}", write_fn);

        STATE = Some(ExploitState {
            process_id: pid,
            thread_id,
            process_hwnd: hwnd,
            base,
            local_shared_memory: local_shared,
            remote_shared_memory: remote_shared,
            set_protect,
            mr_data_addr_ptr,
            mr_data_size_ptr,
            mr_data_addr_orig,
            mr_data_size_orig,
            set_mrprot,
            t,
            w,
            write_fn,
            nt,
            sh,
            ddll,
        });

        PROCESS_BASE.store(base as u64, Ordering::SeqCst);
        LOCAL_SHARED_MEMORY.store(local_shared as u64, Ordering::SeqCst);
        REMOTE_SHARED_MEMORY.store(remote_shared as u64, Ordering::SeqCst);
        INITIALIZED.store(true, Ordering::SeqCst);

        Ok(())
    }
}

pub fn init_driver(pid: u32, _device_name: windows::core::PCSTR) -> Option<()> {
    unsafe {
        let hwnd = find_window_by_pid(pid)?;
        let thread_id = GetWindowThreadProcessId(hwnd, None);
        if thread_id == 0 {
            return None;
        }

        let nt = LoadLibraryA(PCSTR(b"ntdll.dll\0".as_ptr())).ok()?;
        let sh = LoadLibraryA(PCSTR(b"shell32.dll\0".as_ptr())).ok()?;
        let ddll = LoadLibraryA(PCSTR(b"uxtheme.dll\0".as_ptr())).ok()?;

        let local_shared = find_shared_memory_local()?;

        let h_process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;

        let remote_shared =
            find_shared_memory_remote(h_process, thread_id, hwnd, nt, ddll, local_shared)?;

        let base = find_process_base(h_process)?;

        let _ = CloseHandle(h_process);

        let tramp_pattern =
            "48 83 EC ?? 48 8b da 48 85 d2 74 ?? 83 7a ?? ?? 75 ?? 83 7a ?? ?? 75 ??";
        let mut t = pattern_scan_module(sh.0 as *const u8, tramp_pattern)?;
        t = t & 0xFFFFFFFFFFFFFFF0;

        let w = t;

        let set_protect_pattern = "48 8b e9 48 8b d1 41 8d 48 03";
        let mut set_protect = pattern_scan_module(nt.0 as *const u8, set_protect_pattern)?;
        while *(set_protect as *const u8) != 0xCC {
            set_protect -= 1;
        }
        set_protect += 1;

        let mr_data_pattern = "48 8B 05 ?? ?? ?? ?? 4C 8D 44 24 ?? 48 89 44 24";
        let mut set_mrprot = pattern_scan_module(nt.0 as *const u8, mr_data_pattern)?;

        let disp = *((set_mrprot + 3) as *const i32);
        let mr_data_addr_ptr = set_mrprot + disp as usize + 7;
        let mr_data_size_ptr = mr_data_addr_ptr - 0x10;

        let mr_data_addr_orig = *(mr_data_addr_ptr as *const usize);
        let mr_data_size_orig = *(mr_data_size_ptr as *const usize);

        while *(set_mrprot as *const u8) != 0xCC {
            set_mrprot -= 1;
        }
        set_mrprot += 1;

        let write_fn = GetProcAddress(nt, PCSTR(b"RtlCopyString\0".as_ptr()))? as u64;

        STATE = Some(ExploitState {
            process_id: pid,
            thread_id,
            process_hwnd: hwnd,
            base,
            local_shared_memory: local_shared,
            remote_shared_memory: remote_shared,
            set_protect,
            mr_data_addr_ptr,
            mr_data_size_ptr,
            mr_data_addr_orig,
            mr_data_size_orig,
            set_mrprot,
            t,
            w,
            write_fn,
            nt,
            sh,
            ddll,
        });

        PROCESS_BASE.store(base as u64, Ordering::SeqCst);
        LOCAL_SHARED_MEMORY.store(local_shared as u64, Ordering::SeqCst);
        REMOTE_SHARED_MEMORY.store(remote_shared as u64, Ordering::SeqCst);
        INITIALIZED.store(true, Ordering::SeqCst);

        Some(())
    }
}

pub fn process_base() -> usize {
    PROCESS_BASE.load(Ordering::SeqCst) as usize
}

pub fn read_sized(address: usize, size: usize) -> Result<Vec<u8>, MemoryError> {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return Err(MemoryError::NotInitialized);
    }
    if address == 0 {
        return Err(MemoryError::InvalidAddress);
    }
    if size == 0 {
        return Err(MemoryError::InvalidSize);
    }

    let mut result = vec![0u8; size];
    let mut offset = 0;

    while offset < size {
        let chunk_size = (size - offset).min(0x50);
        let chunk = read_data(address + offset, chunk_size)?;
        result[offset..offset + chunk_size].copy_from_slice(&chunk[..chunk_size]);
        offset += chunk_size;
    }

    Ok(result)
}

pub fn writef<T: Copy>(address: usize, value: T) -> Result<(), MemoryError> {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return Err(MemoryError::NotInitialized);
    }
    if address == 0 {
        return Err(MemoryError::InvalidAddress);
    }

    let bytes = unsafe {
        std::slice::from_raw_parts(&value as *const T as *const u8, std::mem::size_of::<T>())
    };
    write_data(address, bytes)
}

fn read_data(address: usize, size: usize) -> Result<Vec<u8>, MemoryError> {
    unsafe {
        #[allow(static_mut_refs)]
        let state = STATE.as_ref().ok_or(MemoryError::NotInitialized)?;

        if size > 0x700 {
            return Err(MemoryError::InvalidSize);
        }

        let _ = ShowWindow(state.process_hwnd, SW_HIDE);
        std::thread::yield_now();

        let a = (state.local_shared_memory + SHARED_MEM_WPARAM_STRUCT) as *mut WParam;
        std::ptr::write_bytes(a as *mut u8, 0, std::mem::size_of::<WParam>());

        (*a).dst_length = size as u16;
        (*a).dst_max_length = size as u16;
        (*a).dst_buffer = (state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT + 0x48) as u64;
        (*a).src_length = size as u16;
        (*a).src_max_length = size as u16;
        (*a).src_buffer = address as u64;
        (*a).arg1 = (state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT + 0x28) as u64;
        (*a).arg2 = (state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT + 0x38) as u64;
        (*a).memcpy = state.write_fn;
        (*a).function_ptr = state.t as u64;
        (*a).lock = 0;

        let hhook = SetWindowsHookExA(
            WH_SHELL,
            Some(std::mem::transmute::<
                usize,
                unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
            >(state.w)),
            Some(HINSTANCE(state.ddll.0)),
            state.thread_id,
        );

        let hhook = match hhook {
            Ok(h) => h,
            Err(_) => {
                let _ = ShowWindow(state.process_hwnd, SW_SHOW);
                return Err(MemoryError::ReadFailed);
            }
        };

        std::thread::yield_now();

        let _ = SendMessageA(
            state.process_hwnd,
            WM_APPCOMMAND,
            WPARAM(state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT),
            LPARAM(rand_u64() as isize),
        );

        let _ = UnhookWindowsHookEx(hhook);
        std::thread::yield_now();

        let _ = ShowWindow(state.process_hwnd, SW_SHOW);

        let mut result = vec![0u8; size];
        std::ptr::copy_nonoverlapping(
            (&(*a).val as *const u64) as *const u8,
            result.as_mut_ptr(),
            size,
        );

        Ok(result)
    }
}

fn write_data(address: usize, data: &[u8]) -> Result<(), MemoryError> {
    let size = data.len();
    let max_size = 0x50;

    if size > max_size {
        let chunks = (size + max_size - 1) / max_size;
        for i in 0..chunks {
            let remainder = size % max_size;
            let chunk_size = if i == chunks - 1 {
                if remainder == 0 {
                    max_size
                } else {
                    remainder
                }
            } else {
                max_size
            };
            write_data(
                address + i * max_size,
                &data[i * max_size..i * max_size + chunk_size],
            )?;
        }
        return Ok(());
    }

    unsafe {
        #[allow(static_mut_refs)]
        let state = STATE.as_ref().ok_or(MemoryError::NotInitialized)?;

        let _ = ShowWindow(state.process_hwnd, SW_HIDE);
        std::thread::yield_now();

        let a = (state.local_shared_memory + SHARED_MEM_WPARAM_STRUCT) as *mut WParam;
        std::ptr::write_bytes(a as *mut u8, 0, std::mem::size_of::<WParam>());

        (*a).dst_length = size as u16;
        (*a).dst_max_length = size as u16;
        (*a).dst_buffer = address as u64;
        (*a).src_length = size as u16;
        (*a).src_max_length = size as u16;
        (*a).src_buffer = (state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT + 0x48) as u64;
        (*a).arg1 = (state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT + 0x28) as u64;
        (*a).arg2 = (state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT + 0x38) as u64;
        (*a).memcpy = state.write_fn;
        (*a).function_ptr = state.t as u64;
        (*a).lock = 0;

        std::ptr::copy_nonoverlapping(data.as_ptr(), (&mut (*a).val as *mut u64) as *mut u8, size);

        let hhook = SetWindowsHookExA(
            WH_SHELL,
            Some(std::mem::transmute::<
                usize,
                unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
            >(state.w)),
            Some(HINSTANCE(state.ddll.0)),
            state.thread_id,
        );

        let hhook = match hhook {
            Ok(h) => h,
            Err(_) => {
                let _ = ShowWindow(state.process_hwnd, SW_SHOW);
                return Err(MemoryError::WriteFailed);
            }
        };

        std::thread::yield_now();

        let _ = SendMessageA(
            state.process_hwnd,
            WM_APPCOMMAND,
            WPARAM(state.remote_shared_memory + SHARED_MEM_WPARAM_STRUCT),
            LPARAM(rand_u64() as isize),
        );

        let _ = UnhookWindowsHookEx(hhook);
        std::thread::yield_now();

        let _ = ShowWindow(state.process_hwnd, SW_SHOW);

        Ok(())
    }
}

unsafe fn find_window_by_pid(pid: u32) -> Option<HWND> {
    struct EnumData {
        target_pid: u32,
        result: Option<HWND>,
    }

    unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let data = &mut *(lparam.0 as *mut EnumData);
        let mut window_pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if window_pid == data.target_pid && IsWindowVisible(hwnd).as_bool() {
            data.result = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    let mut data = EnumData {
        target_pid: pid,
        result: None,
    };

    let _ = EnumWindows(
        Some(enum_callback),
        LPARAM(&mut data as *mut EnumData as isize),
    );
    data.result
}

unsafe fn find_shared_memory_local() -> Option<usize> {
    let h_map = OpenFileMappingA(
        0x0006,
        false,
        PCSTR(b"windows_shell_global_counters\0".as_ptr()),
    );
    if let Ok(h_map) = h_map {
        let view = MapViewOfFile(
            h_map,
            windows::Win32::System::Memory::FILE_MAP(0x0006),
            0,
            0,
            0,
        );
        let _ = CloseHandle(h_map);
        if !view.Value.is_null() {
            return Some(view.Value as usize);
        }
    }

    let h_process = HANDLE(-1isize as *mut _);
    let mut address: *mut u8 = std::ptr::null_mut();
    let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

    let shcore = LoadLibraryA(PCSTR(b"SHCore.dll\0".as_ptr())).ok()?;

    loop {
        if VirtualQueryEx(
            h_process,
            Some(address as *const _),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        ) == 0
        {
            break;
        }

        if mbi.State == MEM_COMMIT
            && mbi.Protect == PAGE_READWRITE
            && mbi.RegionSize == 0x1000
            && mbi.Type == MEM_MAPPED
        {
            let mut address2 = shcore.0 as *mut u8;
            let mut mbi2: MEMORY_BASIC_INFORMATION = std::mem::zeroed();
            let mut i = 0;

            while VirtualQuery(
                Some(address2 as *const _),
                &mut mbi2,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            ) != 0
            {
                if i >= 9 {
                    break;
                }

                if mbi2.Type == MEM_IMAGE && mbi2.Protect == PAGE_READWRITE {
                    let mut ii = 0;
                    while ii < mbi2.RegionSize {
                        if *(address2.add(ii) as *const usize) == mbi.BaseAddress as usize {
                            return Some(mbi.BaseAddress as usize);
                        }
                        ii += 8;
                    }
                }

                i += 1;
                address2 = address2.add(mbi2.RegionSize);
            }
        }

        address = address.add(mbi.RegionSize);
    }

    address = std::ptr::null_mut();
    loop {
        if VirtualQueryEx(
            h_process,
            Some(address as *const _),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        ) == 0
        {
            break;
        }

        if mbi.State == MEM_COMMIT
            && mbi.Protect == PAGE_READWRITE
            && mbi.RegionSize == 0x1000
            && mbi.Type == MEM_MAPPED
        {
            return Some(mbi.BaseAddress as usize);
        }

        address = address.add(mbi.RegionSize);
    }

    None
}

unsafe fn find_shared_memory_remote(
    h_process: HANDLE,
    thread_id: u32,
    hwnd: HWND,
    nt: HMODULE,
    ddll: HMODULE,
    local_shared: usize,
) -> Option<usize> {
    let mut address: *mut u8 = std::ptr::null_mut();
    let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

    loop {
        if VirtualQueryEx(
            h_process,
            Some(address as *const _),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        ) == 0
        {
            break;
        }

        if mbi.State == MEM_COMMIT
            && mbi.Protect == PAGE_READWRITE
            && mbi.RegionSize == 0x1000
            && mbi.Type == MEM_MAPPED
        {
            *((local_shared + 0xF00) as *mut u16) = 0x00;

            let rtl_get_integer_atom = GetProcAddress(nt, PCSTR(b"RtlGetIntegerAtom\0".as_ptr()))?;

            let hhook = SetWindowsHookExA(
                WH_SHELL,
                Some(std::mem::transmute::<
                    unsafe extern "system" fn() -> isize,
                    unsafe extern "system" fn(i32, WPARAM, LPARAM) -> LRESULT,
                >(rtl_get_integer_atom)),
                Some(HINSTANCE(ddll.0)),
                thread_id,
            )
            .ok()?;

            let _ = ShowWindow(hwnd, SW_HIDE);
            std::thread::sleep(std::time::Duration::from_millis(1));

            let _ = SendMessageA(
                hwnd,
                WM_APPCOMMAND,
                WPARAM(address as usize + 0xF00),
                LPARAM((address as usize + 0xF00) as isize),
            );

            std::thread::sleep(std::time::Duration::from_millis(1));
            let _ = UnhookWindowsHookEx(hhook);
            let _ = ShowWindow(hwnd, SW_SHOW);

            if *((local_shared + 0xF00) as *const u16) != 0 {
                return Some(address as usize);
            }
        }

        address = address.add(mbi.RegionSize);
    }

    None
}

unsafe fn find_process_base(h_process: HANDLE) -> Option<usize> {
    let mut address: *mut u8 = std::ptr::null_mut();
    let mut mbi: MEMORY_BASIC_INFORMATION = std::mem::zeroed();

    loop {
        if VirtualQueryEx(
            h_process,
            Some(address as *const _),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        ) == 0
        {
            break;
        }

        if mbi.State == MEM_COMMIT
            && mbi.Protect == PAGE_READONLY
            && mbi.RegionSize == 0x1000
            && mbi.Type == MEM_IMAGE
        {
            let mut filename = [0u8; 1024];
            let len = K32GetMappedFileNameA(h_process, mbi.BaseAddress, &mut filename);
            if len > 0 {
                let name_bytes = &filename[..len as usize];
                if let Ok(name_str) = std::str::from_utf8(name_bytes) {
                    if name_str.contains(".exe") {
                        return Some(address as usize);
                    }
                }
            }
        }

        address = address.add(mbi.RegionSize);
    }

    None
}

fn pattern_scan_module(module: *const u8, signature: &str) -> Option<usize> {
    unsafe {
        let dos_header = module as *const u16;
        if *dos_header != 0x5A4D {
            return None;
        }

        let e_lfanew = *(module.add(0x3C) as *const u32) as usize;
        let nt_headers = module.add(e_lfanew);
        let size_of_image = *(nt_headers.add(0x18 + 0x38) as *const u32) as usize;

        let pattern_bytes = pattern_to_bytes(signature);
        let pattern_len = pattern_bytes.len();

        if size_of_image < pattern_len {
            return None;
        }

        for i in 0..(size_of_image - pattern_len) {
            let mut found = true;
            for (j, byte) in pattern_bytes.iter().enumerate() {
                if let Some(b) = byte {
                    if *module.add(i + j) != *b {
                        found = false;
                        break;
                    }
                }
            }
            if found {
                return Some(module.add(i) as usize);
            }
        }

        None
    }
}

fn pattern_to_bytes(pattern: &str) -> Vec<Option<u8>> {
    let mut bytes = Vec::new();
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '?' {
            if chars.peek() == Some(&'?') {
                chars.next();
            }
            bytes.push(None);
        } else if c == ' ' {
            continue;
        } else {
            let hex_str: String = std::iter::once(c).chain(chars.by_ref().take(1)).collect();
            if let Ok(byte) = u8::from_str_radix(&hex_str, 16) {
                bytes.push(Some(byte));
            }
        }
    }

    bytes
}

fn rand_u64() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    seed.wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407)
}
