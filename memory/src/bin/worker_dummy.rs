use std::io::Write;

use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageA, GetMessageA, PeekMessageA, TranslateMessage, MSG, PM_NOREMOVE,
};

static READ_SENTINEL: [u8; 16] = *b"wsw-dummy-read!!";
static mut WRITE_SENTINEL: u64 = 0x1122_3344_5566_7788;

fn main() {
    unsafe {
        let mut msg = MSG::default();
        let _ = PeekMessageA(&mut msg, None, 0, 0, PM_NOREMOVE);

        println!(
            "READY {} {} {:#x} {:#x}",
            GetCurrentProcessId(),
            GetCurrentThreadId(),
            READ_SENTINEL.as_ptr() as usize,
            core::ptr::addr_of_mut!(WRITE_SENTINEL) as usize
        );
        let _ = std::io::stdout().flush();

        while GetMessageA(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageA(&msg);
        }
    }
}
