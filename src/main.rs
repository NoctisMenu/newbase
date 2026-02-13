#![windows_subsystem = "windows"] // hide console window on Windows in release
#![feature(stmt_expr_attributes, const_cmp, const_trait_impl)]
#![allow(dead_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("This application only supports Windows OS!");

const PROCESS_NAME: &str = "deadlock.exe";
#[cfg(feature = "launch_game")]
const APP_ID: u32 = 1422450;

const _: () = {
    assert!(PROCESS_NAME != "", "PROCESS_NAME cannot be empty!");
    assert!(APP_ID != 0, "APP_ID cannot be 0!");
};

use std::{
    process::Command,
    sync::{Arc, atomic::AtomicI64},
};

//extern imports
use windows_strings::s;

//crate defs
mod app;
mod datatypes;
pub use app::*;
pub use datatypes::*;
pub use memory::memory::*;
mod macros;
mod models;
mod overlay;

pub use models::*;

// Setup panic hook to catch panics and log them before exiting
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
        std::thread::sleep(std::time::Duration::from_secs(10));
        std::process::exit(1);
    }));
}

#[cfg(feature = "launch_game")]
fn launch_game() {
    use std::{os::windows::process::CommandExt, process::Command};
    let steam_url = format!("steam://rungameid/{}", APP_ID);

    //who gaf
    let _ = Command::new("cmd")
        .args(&["/C", "start", "", &steam_url])
        .creation_flags(0x08000000)
        .spawn();
}

fn copy_driver() {
    if !std::fs::exists("C:\\Windows\\System32\\drivers\\WinNotify.sys").unwrap() {
        let driver_bytes = include_bytes!("../WinNotify.sys");
        let _ = std::fs::write(
            "C:\\Windows\\System32\\drivers\\WinNotify.sys",
            driver_bytes,
        );
    }

    let sc_query = Command::new("sc").args(&["query", "WinNotify"]).output();
    if let Ok(output) = sc_query {
        let output_str = String::from_utf8_lossy(&output.stdout);
        if output_str.contains("FAILED") {
            let sc_create = Command::new("sc")
                .args(&[
                    "create",
                    "WinNotify",
                    "type=",
                    "kernel",
                    "start=",
                    "demand",
                    "binPath=",
                    r"C:\Windows\System32\drivers\WinNotify.sys",
                ])
                .output();
            if let Ok(_) = sc_create {
                log::info!("Driver service created successfully.");
            } else {
                log::error!("Failed to create driver service.");
                std::process::exit(1);
            }
        } else {
            log::info!("Driver service already exists.");
        }
        let sc_start = Command::new("sc").args(&["start", "WinNotify"]).output();
        if let Ok(_) = sc_start {
            log::info!("Driver service started successfully.");
        } else {
            log::error!("Failed to start driver service.");
        }
    }
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

        // Generate random title
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
        title_wide.push(0); // Null terminator
        let _ = SetConsoleTitleW(windows::core::PCWSTR(title_wide.as_ptr()));

        // adjust as much as you want but 50-75 is good to allow shit to happen
        std::thread::sleep(std::time::Duration::from_millis(75));

        let hwnd = GetConsoleWindow();
        if hwnd.is_invalid() {
            println!("fucked off??");
            return;
        }

        let style = (WS_POPUP | WS_VISIBLE).0 as isize;
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, style);
        let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, 0);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            100, //x
            100, //y
            800, //w
            600, //h
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

        //zero fucking idea why with just SB_BOTH it only hides the scroll bare like 80% of the time so more the merry
        //cuz with all 3 it never shows, windows being sped as fuck
        let _ = ShowScrollBar(hwnd, SB_BOTH, false);
        let _ = ShowScrollBar(hwnd, SB_HORZ, false);
        let _ = ShowScrollBar(hwnd, SB_VERT, false);
    }
}

fn main() {
    // Debugging initialization
    colog::init();

    // Setup panic hook to catch and log panics from all threads
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
        let _ = launch_game();
    }

    let mut cur_loops = 0;
    let pid = loop {
        match memory::driver::return_pid(PROCESS_NAME) {
            Some(pid) => {
                log::info!("Found game process...");
                break pid;
            }
            None => {
                log::warn!("Failed to find game process!");
                std::thread::sleep(std::time::Duration::from_millis(5000));
                cur_loops += 1;
                if cur_loops > 20 {
                    log::error!("Failed to find game process after 20 loops!");
                    return;
                }
                continue;
            }
        }
    };

    let _time_remaining = time_remaining.clone();
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
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    });

    cur_loops = 0;
    let window = loop {
        match windowing::find_window_by_pid(pid) {
            Ok(Some(window)) => {
                log::info!("Found game window");
                break window;
            }
            _ => {
                log::warn!("Searching for game window...");
                std::thread::sleep(std::time::Duration::from_millis(5000));
                cur_loops += 1;
                if cur_loops > 20 {
                    log::error!("Failed to find game window after 20 loops!");
                    return;
                }
                continue;
            }
        }
    };
    copy_driver();
    match init_driver(pid, s!("\\\\.\\WinNotify")) {
        Some(_) => {
            log::info!("Successfully initialized...");
        }
        None => {
            log::error!("0xD1; are you running as admin?");
            return;
        }
    };
    #[cfg(not(debug_assertions))]
    unsafe {
        let _ = windows::Win32::System::Console::FreeConsole();
    }

    log::info!("Entering main app loop!");
    App::start(pid, window, time_remaining);
}
