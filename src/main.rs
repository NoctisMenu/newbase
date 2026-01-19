#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![feature(stmt_expr_attributes,const_cmp,const_trait_impl)]
#![allow(dead_code)]

const PROCESS_NAME: &str = "Notepad.exe";
#[cfg(feature = "launch_game")]
const APP_ID: u32 = 1;
const AUTHERIUM_PRODUCT_ID: &str = "a";

const AUTHERIUM_URL: &str = "https://noctismenu.dev";
const LOADER_DISCORD: &str = "https://discord.com";
const LOADER_WEBSITE: &str = "https://noctismenu.dev";
const LOADER_WINDOW_NAME: &str = "noctis";
const DRIVER_NAME: PCSTR = s!("\\\\.\\WinNotify");

const _: () = {
    assert!(PROCESS_NAME != "", "PROCESS_NAME cannot be empty!");
    assert!(APP_ID != 0, "APP_ID cannot be 0!");
    assert!(AUTHERIUM_PRODUCT_ID != "", "AUTHERIUM_PRODUCT_ID cannot be empty!");
};

//shouldn't really change
pub const DISCORD_APP_ID: i64 = 1434438467064168501;

use std::{
    process::Command,
    sync::{Arc, atomic::AtomicI64},
};
//extern imports
use windows_strings::{PCSTR, s};

//crate defs
mod app;
mod datatypes;
pub use app::*;
pub use datatypes::*;
mod widgets;
pub use memory::memory::*;
pub use widgets::*;
mod models;
pub use models::*;
mod discord;

#[cfg(feature = "launch_game")]
fn launch_game() -> std::io::Result<()> {
    let steam_url = format!("steam://rungameid/{}", APP_ID);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        Command::new("cmd")
            .args(&["/C", "start", "", &steam_url])
            .creation_flags(0x08000000)
            .spawn()?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        return Err(std::io::ErrorKind::Unsupported.into());
    }

    Ok(())
}

fn main() {
    // Debugging initialization
    #[cfg(debug_assertions)]
    colog::init();
    let start = std::time::Instant::now();

    let self_pid = std::process::id();
    log::info!("Self PID: {}", self_pid);

    //do license loader n shit
    let time_remaining = Arc::new(AtomicI64::new(100000));
    #[cfg(all(feature = "loader", not(debug_assertions)))]
    {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
        log::info!("Starting loader...");
        autherium_loader::loader::start::start(
            LOADER_WINDOW_NAME,
            AUTHERIUM_URL,
            AUTHERIUM_PRODUCT_ID,
            LOADER_DISCORD,
            LOADER_WEBSITE,
            Some(Arc::clone(&time_remaining)),
        );
        //because our loader is fucking stupid we have to hide the window after otherwise it freezes and looks bad

        if let Ok(Some(hwnd)) = windowing::find_window_by_pid(self_pid) {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    #[cfg(feature = "launch_game")]
    if memory::driver::return_pid(PROCESS_NAME).is_none() {
        log::info!("Launching game...");
        let _ = launch_game();
    } else {
        log::info!("Game is already launched!");
    }
    let mut cur_loops = 0;

    let pid = loop {
        match memory::driver::return_pid(PROCESS_NAME) {
            Some(pid) => {
                log::info!("Found process ID: {}", pid);
                break pid;
            }
            None => {
                log::warn!("Failed to find process ID!");
                std::thread::sleep(std::time::Duration::from_millis(1000));
                cur_loops += 1;
                if cur_loops > 20 {
                    log::error!("Failed to find process ID after 20 loops!");
                    autherium_loader::loader::start::error(
                        LOADER_WINDOW_NAME,
                        "Failed to find game process!",
                    );
                    return;
                }
                continue;
            }
        }
    };
    cur_loops = 0;
    let window = loop {
        match windowing::find_window_by_pid(pid) {
            Ok(Some(window)) => {
                log::info!("Found window handle: {:?}", window);
                break window;
            }
            _ => {
                log::warn!("Failed to find window handle!");
                std::thread::sleep(std::time::Duration::from_millis(1000));
                cur_loops += 1;
                if cur_loops > 20 {
                    log::error!("Failed to find game window after 20 loops!");
                    autherium_loader::loader::start::error(
                        LOADER_WINDOW_NAME,
                        "Failed to find game window!",
                    );
                    return;
                }
                continue;
            }
        }
    };
    match init_driver(pid, DRIVER_NAME) {
        Some(_) => {
            log::info!("Successfully acquired handle to kernel driver");
        }
        None => {
            log::error!("Failed to acquire kernel handle!");
            autherium_loader::loader::start::error(
                LOADER_WINDOW_NAME,
                "Failed to acquire handle to km driver!",
            );
            return;
        }
    };
    std::thread::spawn(|| {
        loop {
            if memory::driver::return_pid(PROCESS_NAME).is_none() {
                std::process::exit(0);
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    });
    log::info!("Entering main app loop!");
    App::start(self_pid, pid, window, time_remaining, start);
}
