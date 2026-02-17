#![windows_subsystem = "windows"] // hide console window on Windows in release
#![allow(dead_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("This application only supports Windows OS!");

use newbase::{App, LogicSystem, ThreadCtx, ThreadFlow, logic_system};
use std::sync::Mutex;

#[derive(Default)]
struct AppData {
    pub player_buf: Mutex<Vec<i32>>,
}

#[logic_system(name = "esp")]
fn esp(
    app: &mut App<AppData>,
    ui: &newoverlay::imgui::Ui,
    draw_list: &newoverlay::imgui::DrawListMut,
) {
    dbg!("called every tick");
}

fn cache_players(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    let state = ctx.state();
    state.player_buf.lock().unwrap().clear();
    ThreadFlow::Continue
}


fn main() {
    newbase::init::custom_builder(AppData::default())
        .expect("Failed to initialize runtime")
        .with_logic(Aimbot)
        .with_thread(
            "players",
            |x| cache_players(x)
        )
        .run();
}
