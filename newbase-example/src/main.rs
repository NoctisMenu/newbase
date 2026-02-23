//#![windows_subsystem = "windows"] // hide console window on Windows in release
#![allow(dead_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("This application only supports Windows OS!");
pub mod config_system;
mod offsets;
pub use config_system::*;

mod aimbot;
mod esp;
mod models;
mod player;
mod scripting;

use crate::player::AppData;

const SCHEMA_TOML: &str = include_str!("../config_schema.toml");

fn main() {
    //colog::init();
    newbase::init::custom_builder(AppData::default(), "deadlock.exe", Some(1422450))
        .expect("Failed to initialize runtime")
        .with_logic(esp::system())
        .with_config_schema_str(SCHEMA_TOML, "config.toml")
        .with_thread("players", esp::players)
        .with_thread("actors", esp::entities)
        .with_thread("aimbot", aimbot::run)
        .with_thread("lua", scripting::run)
        .run();
}
