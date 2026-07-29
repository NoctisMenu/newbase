use std::ffi::CString;
use std::mem;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use windows::core::PCSTR;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HINSTANCE, HMODULE, INVALID_HANDLE_VALUE, LPARAM, WPARAM,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};
use windows::Win32::System::Memory::{
    CreateFileMappingA, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS, PAGE_READWRITE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PostThreadMessageA, SetWindowsHookExA, UnhookWindowsHookEx, HHOOK, WH_GETMESSAGE, WM_NULL,
};

use crate::types::MemoryError;

const STATE_STARTING: u32 = 0;
const STATE_READY: u32 = 1;
const STATE_PENDING: u32 = 2;
const STATE_COMPLETE: u32 = 3;
const STATE_STOP: u32 = 4;
const STATE_STOPPED: u32 = 5;

const OP_READ: u32 = 0;
const OP_WRITE: u32 = 1;

pub const MAPPING_SIZE: usize = 64 * 1024;
const HEADER_SIZE: usize = 32;
const DATA_SIZE: usize = MAPPING_SIZE - HEADER_SIZE;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[repr(C)]
struct SharedCmd {
    state: AtomicU32,
    op: u32,
    status: u32,
    _reserved: u32,
    addr: u64,
    size: u64,
}

pub struct WorkerDriver {
    pid: u32,
    thread_id: u32,
    mapping: HANDLE,
    view: *mut u8,
    hook: HHOOK,
    _local_module: HMODULE,
}

unsafe impl Send for WorkerDriver {}
unsafe impl Sync for WorkerDriver {}

impl WorkerDriver {
    pub fn attach_to_thread(
        pid: u32,
        thread_id: u32,
        dll_path: impl AsRef<Path>,
    ) -> Result<Self, MemoryError> {
        unsafe {
            let section_name = CString::new(format!("Local\\wsw_{pid}"))
                .map_err(|_| MemoryError::InvalidAddress)?;
            let mapping = CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                0,
                MAPPING_SIZE as u32,
                PCSTR(section_name.as_ptr() as *const u8),
            )
            .map_err(|_| MemoryError::SharedMemoryNotFound)?;

            let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, MAPPING_SIZE);
            if view.Value.is_null() {
                let _ = CloseHandle(mapping);
                return Err(MemoryError::SharedMemoryNotFound);
            }

            let this = (view.Value as *mut u8) as *mut SharedCmd;
            core::ptr::write_bytes(view.Value as *mut u8, 0, MAPPING_SIZE);
            (*this).state.store(STATE_STARTING, Ordering::Release);

            let dll_path = dll_path.as_ref().as_os_str().to_string_lossy().into_owned();
            let dll_path = CString::new(dll_path).map_err(|_| MemoryError::InvalidAddress)?;
            let local_module = LoadLibraryA(PCSTR(dll_path.as_ptr() as *const u8))
                .map_err(|_| MemoryError::InjectionFailed)?;
            let hook_proc = GetProcAddress(local_module, PCSTR(b"HookProc\0".as_ptr()))
                .ok_or(MemoryError::InjectionFailed)?;
            let hook_proc = mem::transmute(hook_proc);
            let hook = SetWindowsHookExA(
                WH_GETMESSAGE,
                hook_proc,
                Some(HINSTANCE(local_module.0)),
                thread_id,
            )
            .map_err(|_| MemoryError::InjectionFailed)?;

            let driver = Self {
                pid,
                thread_id,
                mapping,
                view: view.Value as *mut u8,
                hook,
                _local_module: local_module,
            };

            driver.wait_for_ready(DEFAULT_TIMEOUT)?;
            Ok(driver)
        }
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    pub fn max_transfer_size(&self) -> usize {
        DATA_SIZE
    }

    pub fn read_bytes(&self, address: usize, size: usize) -> Result<Vec<u8>, MemoryError> {
        if address == 0 {
            return Err(MemoryError::InvalidAddress);
        }
        if size == 0 || size > DATA_SIZE {
            return Err(MemoryError::InvalidSize);
        }

        unsafe {
            let cmd = self.cmd();
            self.wait_for_state(STATE_READY, DEFAULT_TIMEOUT)?;
            (*cmd).op = OP_READ;
            (*cmd).status = 0;
            (*cmd).addr = address as u64;
            (*cmd).size = size as u64;
            (*cmd).state.store(STATE_PENDING, Ordering::Release);

            self.wait_for_state(STATE_COMPLETE, DEFAULT_TIMEOUT)?;
            if (*cmd).status != 0 {
                (*cmd).state.store(STATE_READY, Ordering::Release);
                return Err(MemoryError::ReadFailed);
            }

            let mut out = vec![0u8; size];
            out.copy_from_slice(std::slice::from_raw_parts(self.data(), size));
            (*cmd).state.store(STATE_READY, Ordering::Release);
            Ok(out)
        }
    }

    pub fn read<T: Copy>(&self, address: usize) -> Result<T, MemoryError> {
        let bytes = self.read_bytes(address, mem::size_of::<T>())?;
        unsafe { Ok(core::ptr::read_unaligned(bytes.as_ptr() as *const T)) }
    }

    pub fn write_bytes(&self, address: usize, bytes: &[u8]) -> Result<(), MemoryError> {
        if address == 0 {
            return Err(MemoryError::InvalidAddress);
        }
        if bytes.is_empty() || bytes.len() > DATA_SIZE {
            return Err(MemoryError::InvalidSize);
        }

        unsafe {
            let cmd = self.cmd();
            self.wait_for_state(STATE_READY, DEFAULT_TIMEOUT)?;
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), self.data(), bytes.len());
            (*cmd).op = OP_WRITE;
            (*cmd).status = 0;
            (*cmd).addr = address as u64;
            (*cmd).size = bytes.len() as u64;
            (*cmd).state.store(STATE_PENDING, Ordering::Release);

            self.wait_for_state(STATE_COMPLETE, DEFAULT_TIMEOUT)?;
            let ok = (*cmd).status == 0;
            (*cmd).state.store(STATE_READY, Ordering::Release);
            if ok {
                Ok(())
            } else {
                Err(MemoryError::WriteFailed)
            }
        }
    }

    pub fn write<T: Copy>(&self, address: usize, value: T) -> Result<(), MemoryError> {
        let bytes = unsafe {
            std::slice::from_raw_parts((&value as *const T) as *const u8, mem::size_of::<T>())
        };
        self.write_bytes(address, bytes)
    }

    pub fn shutdown(&self) -> Result<(), MemoryError> {
        unsafe {
            let cmd = self.cmd();
            let state = (*cmd).state.load(Ordering::Acquire);
            if state == STATE_STOPPED {
                return Ok(());
            }
            (*cmd).state.store(STATE_STOP, Ordering::Release);
            self.wait_for_state(STATE_STOPPED, DEFAULT_TIMEOUT)
        }
    }

    unsafe fn cmd(&self) -> *mut SharedCmd {
        self.view as *mut SharedCmd
    }

    unsafe fn data(&self) -> *mut u8 {
        self.view.add(HEADER_SIZE)
    }

    fn wait_for_ready(&self, timeout: Duration) -> Result<(), MemoryError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            unsafe {
                let state = (*self.cmd()).state.load(Ordering::Acquire);
                if state == STATE_READY {
                    return Ok(());
                }
            }

            unsafe {
                let _ = PostThreadMessageA(self.thread_id, WM_NULL, WPARAM(0), LPARAM(0));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(MemoryError::Timeout)
    }

    fn wait_for_state(&self, expected: u32, timeout: Duration) -> Result<(), MemoryError> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            unsafe {
                if (*self.cmd()).state.load(Ordering::Acquire) == expected {
                    return Ok(());
                }
            }
            core::hint::spin_loop();
        }
        Err(MemoryError::Timeout)
    }
}

impl Drop for WorkerDriver {
    fn drop(&mut self) {
        let _ = self.shutdown();
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
            let _ = UnmapViewOfFile(windows::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.view.cast(),
            });
            let _ = CloseHandle(self.mapping);
        }
    }
}
