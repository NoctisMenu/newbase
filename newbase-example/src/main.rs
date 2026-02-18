#![windows_subsystem = "windows"] // hide console window on Windows in release
#![allow(dead_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("This application only supports Windows OS!");

pub mod config_system;
pub use config_system::*;

mod esp;
mod aimbot;
mod models;
mod offsets;
mod player;

use crate::player::AppData;

const SCHEMA_TOML: &str = include_str!("../config_schema.toml");

fn main() {
    newbase::init::custom_builder(AppData::default(), "deadlock.exe", Some(1422450))
        .expect("Failed to initialize runtime")
        .with_logic(esp::system())
        .with_config_schema_str(SCHEMA_TOML, "config.toml")
        .with_thread("players", esp::cache_players)
        .with_thread("aimbot", aimbot::run)
        .run();
}
