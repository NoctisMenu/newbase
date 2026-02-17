//#![windows_subsystem = "windows"] // hide console window on Windows in release
fn main() {
    newbase::init::custom_builder({},"deadlock.exe",Some(1422450))
        .expect("Failed to initialize runtime")
        .run();
}