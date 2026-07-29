use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingA, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
};
use windows::Win32::System::Threading::{CreateThread, GetCurrentProcessId};
use windows::Win32::UI::WindowsAndMessaging::CallNextHookEx;

const STATE_READY: u32 = 1;
const STATE_PENDING: u32 = 2;
const STATE_COMPLETE: u32 = 3;
const STATE_STOP: u32 = 4;
const STATE_STOPPED: u32 = 5;
const OP_READ: u32 = 0;
const OP_WRITE: u32 = 1;
const MAPPING_SIZE: usize = 64 * 1024;
const HEADER_SIZE: usize = 32;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[repr(C)]
struct SharedCmd {
    state: AtomicU32,
    op: u32,
    status: u32,
    _reserved: u32,
    addr: u64,
    size: u64,
}

unsafe extern "system" fn worker_thread(param: *mut c_void) -> u32 {
    let base = param as *mut u8;
    let cmd = base as *mut SharedCmd;
    let data = base.add(HEADER_SIZE);

    (*cmd).state.store(STATE_READY, Ordering::Release);

    loop {
        let state = (*cmd).state.load(Ordering::Acquire);
        if state == STATE_STOP {
            break;
        }
        if state != STATE_PENDING {
            core::hint::spin_loop();
            continue;
        }

        let addr = (*cmd).addr as *mut u8;
        let size = (*cmd).size as usize;
        if size > MAPPING_SIZE - HEADER_SIZE {
            (*cmd).status = 1;
        } else if (*cmd).op == OP_READ {
            core::ptr::copy_nonoverlapping(addr, data, size);
            (*cmd).status = 0;
        } else if (*cmd).op == OP_WRITE {
            core::ptr::copy_nonoverlapping(data, addr, size);
            (*cmd).status = 0;
        } else {
            (*cmd).status = 2;
        }

        (*cmd).state.store(STATE_COMPLETE, Ordering::Release);
    }

    (*cmd).state.store(STATE_STOPPED, Ordering::Release);
    0
}

#[no_mangle]
pub unsafe extern "system" fn HookProc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && !INITIALIZED.swap(true, Ordering::AcqRel) {
        let pid = GetCurrentProcessId();
        let name = format!("Local\\wsw_{pid}\0");

        if let Ok(mapping) = OpenFileMappingA(FILE_MAP_ALL_ACCESS.0, false, PCSTR(name.as_ptr())) {
            let view = MapViewOfFile(mapping, FILE_MAP_ALL_ACCESS, 0, 0, MAPPING_SIZE);
            if !view.Value.is_null() {
                if let Ok(thread) = CreateThread(
                    None,
                    0,
                    Some(worker_thread),
                    Some(view.Value.cast_const()),
                    Default::default(),
                    None,
                ) {
                    let _ = CloseHandle(thread);
                } else {
                    let _ = UnmapViewOfFile(view);
                    let _ = CloseHandle(mapping);
                }
            } else {
                let _ = CloseHandle(mapping);
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[no_mangle]
pub extern "system" fn DllMain(_instance: HINSTANCE, _reason: u32, _reserved: *mut c_void) -> i32 {
    1
}
