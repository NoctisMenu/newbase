#![feature(stmt_expr_attributes)]
#![allow(dead_code)]

extern crate composite as newoverlay;

#[cfg(not(target_os = "windows"))]
compile_error!("This library only supports Windows OS!");

pub mod app;
pub mod build_support;
pub mod init;
pub mod input;
mod macros;
mod models;
mod overlay;

pub use app::*;
pub use input::{InputBackend, InputDeviceState, InputError, MouseButton};
pub use logic_system_macros::logic_system;
pub use memory::memory::*;
