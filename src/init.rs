use std::{
    io,
    process::Command,
    sync::{Arc, atomic::AtomicI64},
    time::Duration,
};

use thiserror::Error;
use windows_strings::s;

use crate::{App, AppBuilder};

const PROCESS_NAME: &str = "deadlock.exe";
#[cfg(feature = "launch_game")]
const APP_ID: u32 = 1422450;
const MAX_RETRIES: u32 = 20;
const RETRY_DELAY: Duration = Duration::from_millis(5000);

#[derive(Debug, Error)]
pub enum InitError {
    #[error("failed to find game process '{process_name}' after {retries} retries")]
    ProcessNotFound {
        process_name: &'static str,
        retries: u32,
    },
    #[error("failed to find game window for pid {pid} after {retries} retries")]
    WindowNotFound { pid: u32, retries: u32 },
    #[error("failed to initialize driver; are you running as admin?")]
    DriverInitFailed,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}

/// Initialize runtime, resolve process/window/driver state, and return a ready AppBuilder.
///
/// After this returns, users only need to call `.with_logic(...)` and `.run()`.
pub fn custom_builder<S: Send + Sync + 'static>(state: S) -> Result<AppBuilder<S>, InitError> {
    colog::init();

    #[cfg(debug_assertions)]
    setup_panic_hook();

    disable_console_decorations();

    let time_remaining = Arc::new(AtomicI64::new(
        std::env::var("WINVER_ID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(-1),
    ));

    #[cfg(feature = "launch_game")]
    if memory::driver::return_pid(PROCESS_NAME).is_none() {
        log::info!("Launching game...");
        launch_game();
    }

    let pid = wait_for_pid(PROCESS_NAME, MAX_RETRIES)?;
    spawn_watchdog_thread(time_remaining.clone());
    let window = wait_for_window(pid, MAX_RETRIES)?;

    copy_driver()?;

    if crate::init_driver(pid, s!("\\\\.\\WinNotify")).is_none() {
        return Err(InitError::DriverInitFailed);
    }
    log::info!("Successfully initialized...");

    #[cfg(not(debug_assertions))]
    unsafe {
        let _ = windows::Win32::System::Console::FreeConsole();
    }

    log::info!("Entering main app loop!");
    Ok(App::builder(pid, window, time_remaining, state))
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "An unknown error occurred.".to_string()
        };

        let location = panic_info
            .location()
            .map(|loc| format!(" at {}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_default();

        log::error!("Panic occurred: {}{}", message, location);
        std::thread::sleep(Duration::from_secs(10));
        std::process::exit(1);
    }));
}

#[cfg(feature = "launch_game")]
fn launch_game() {
    use std::os::windows::process::CommandExt;
    let steam_url = format!("steam://rungameid/{}", APP_ID);
    let _ = Command::new("cmd")
        .args(["/C", "start", "", &steam_url])
        .creation_flags(0x08000000)
        .spawn();
}

fn copy_driver() -> Result<(), InitError> {
    let destination = "C:\\Windows\\System32\\drivers\\WinNotify.sys";
    if !std::fs::exists(destination)? {
        let driver_bytes = include_bytes!("../resources/WinNotify.sys");
        std::fs::write(destination, driver_bytes)?;
    }

    if let Ok(output) = Command::new("sc").args(["query", "WinNotify"]).output() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if output_str.contains("FAILED") {
            let sc_create = Command::new("sc")
                .args([
                    "create",
                    "WinNotify",
                    "type=",
                    "kernel",
                    "start=",
                    "demand",
                    "binPath=",
                    destination,
                ])
                .output();
            if sc_create.is_ok() {
                log::info!("Driver service created successfully.");
            } else {
                log::error!("Failed to create driver service.");
                return Err(InitError::DriverInitFailed);
            }
        } else {
            log::info!("Driver service already exists.");
        }

        let sc_start = Command::new("sc").args(["start", "WinNotify"]).output();
        if sc_start.is_ok() {
            log::info!("Driver service started successfully.");
        } else {
            log::error!("Failed to start driver service.");
        }
    }

    Ok(())
}

fn wait_for_pid(process_name: &'static str, max_retries: u32) -> Result<u32, InitError> {
    let mut retries = 0;
    loop {
        match memory::driver::return_pid(process_name) {
            Some(pid) => {
                log::info!("Found game process...");
                return Ok(pid);
            }
            None => {
                log::warn!("Failed to find game process!");
                if retries >= max_retries {
                    return Err(InitError::ProcessNotFound {
                        process_name,
                        retries: max_retries,
                    });
                }
                retries += 1;
                std::thread::sleep(RETRY_DELAY);
            }
        }
    }
}

fn wait_for_window(pid: u32, max_retries: u32) -> Result<windowing::Window, InitError> {
    let mut retries = 0;
    loop {
        match windowing::find_window_by_pid(pid) {
            Ok(Some(window)) => {
                log::info!("Found game window");
                return Ok(window);
            }
            _ => {
                log::warn!("Searching for game window...");
                if retries >= max_retries {
                    return Err(InitError::WindowNotFound {
                        pid,
                        retries: max_retries,
                    });
                }
                retries += 1;
                std::thread::sleep(RETRY_DELAY);
            }
        }
    }
}

fn spawn_watchdog_thread(_time_remaining: Arc<AtomicI64>) {
    std::thread::spawn(move || {
        loop {
            if memory::driver::return_pid(PROCESS_NAME).is_none() {
                std::process::exit(0);
            }
            #[cfg(not(debug_assertions))]
            if _time_remaining.load(std::sync::atomic::Ordering::SeqCst) < 0 {
                log::error!("License time expired, exiting!");
                std::process::exit(0);
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    });
}

fn disable_console_decorations() {
    use rand::Rng;
    use windows::Win32::System::Console::{
        AllocConsole, CONSOLE_SCREEN_BUFFER_INFO, GetConsoleScreenBufferInfo, GetConsoleWindow,
        GetStdHandle, STD_OUTPUT_HANDLE, SetConsoleScreenBufferSize, SetConsoleTitleW,
    };
    use windows::Win32::UI::Controls::ShowScrollBar;
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, HWND_TOP, SB_BOTH, SB_HORZ, SB_VERT, SWP_FRAMECHANGED,
        SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, WS_POPUP, WS_VISIBLE,
    };

    unsafe {
        let _ = AllocConsole();

        let mut rng = rand::rng();
        let random_title: String = (0..16)
            .map(|_| {
                let idx = rng.random_range(0..62);
                match idx {
                    0..=9 => (b'0' + idx) as char,
                    10..=35 => (b'a' + (idx - 10)) as char,
                    _ => (b'A' + (idx - 36)) as char,
                }
            })
            .collect();

        let mut title_wide: Vec<u16> = random_title.encode_utf16().collect();
        title_wide.push(0);
        let _ = SetConsoleTitleW(windows::core::PCWSTR(title_wide.as_ptr()));

        std::thread::sleep(Duration::from_millis(75));

        let hwnd = GetConsoleWindow();
        if hwnd.is_invalid() {
            return;
        }

        let style = (WS_POPUP | WS_VISIBLE).0 as isize;
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, style);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, 0);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            100,
            100,
            800,
            600,
            SWP_FRAMECHANGED | SWP_SHOWWINDOW,
        );

        let stdout = GetStdHandle(STD_OUTPUT_HANDLE).unwrap();
        let mut csbi = CONSOLE_SCREEN_BUFFER_INFO::default();
        if GetConsoleScreenBufferInfo(stdout, &mut csbi).is_ok() {
            let ww = csbi.srWindow.Right - csbi.srWindow.Left + 1;
            let wh = csbi.srWindow.Bottom - csbi.srWindow.Top + 1;
            let ns = windows::Win32::System::Console::COORD { X: ww, Y: wh };
            let _ = SetConsoleScreenBufferSize(stdout, ns);
        }

        let _ = ShowScrollBar(hwnd, SB_BOTH, false);
        let _ = ShowScrollBar(hwnd, SB_HORZ, false);
        let _ = ShowScrollBar(hwnd, SB_VERT, false);
    }
}
