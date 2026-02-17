#![feature(stmt_expr_attributes, const_cmp, const_trait_impl)]
#![allow(dead_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("This library only supports Windows OS!");

pub mod app;
pub mod build_support;
pub mod init;
mod macros;
mod models;
mod overlay;

pub use app::*;
pub use logic_system_macros::logic_system;
pub use memory::memory::*;
