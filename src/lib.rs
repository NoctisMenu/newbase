#![feature(stmt_expr_attributes)]
#![allow(dead_code)]

extern crate composite as newoverlay;

#[cfg(not(target_os = "windows"))]
compile_error!("This library only supports Windows OS!");

pub mod app;
pub mod build_support;
pub mod init;
mod macros;
mod models;
mod overlay;
#[cfg(target_arch = "x86_64")]
mod remote;
#[cfg(target_arch = "x86_64")]
pub mod remote_hook;

pub use app::*;
pub use logic_system_macros::logic_system;
pub use memory::memory::*;
#[cfg(target_arch = "x86_64")]
pub use remote::{
    MAX_REMOTE_ARGUMENTS, RemoteArgument, RemoteCallError, RemoteCallResult, RemoteProcess,
};
#[cfg(target_arch = "x86_64")]
pub use remote_hook::{RemoteHook, RemoteHookError, RemoteHookKind};
