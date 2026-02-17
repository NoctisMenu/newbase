#![windows_subsystem = "windows"] // hide console window on Windows in release
#![allow(dead_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("This application only supports Windows OS!");

use newbase::{App, LogicSystem, ThreadCtx, ThreadFlow, logic_system, read, skip_err, skip_opt};
use std::sync::Mutex;

pub mod config_system;
pub use config_system::*;

mod models;
pub use models::math::*;

const SCHEMA_TOML: &str = include_str!("../config_schema.toml");

#[derive(Default)]
struct AppData {
    pub player_buf: Mutex<Vec<i32>>,
}

use pelite::pattern;
use pelite::pattern::{Atom, save_len};
use pelite::pe64::{Pe, PeView, Rva};

use phf::{Map, phf_map};

use std::collections::BTreeMap;
use std::sync::{LazyLock, OnceLock};

use log::{debug, error};

use anyhow::Result;
pub type OffsetMap = BTreeMap<String, BTreeMap<String, Rva>>;

macro_rules! pattern_map {
    ($($module:ident => {
        $($name:expr => $pattern:expr $(=> $callback:expr)?),+ $(,)?
    }),+ $(,)?) => {
        $(
            mod $module {
                use super::*;

                pub(super) const PATTERNS: Map<
                    &'static str,
                    (
                        &'static [Atom],
                        Option<fn(&PeView, &mut BTreeMap<String, Rva>, Rva)>,
                    ),
                > = phf_map! {
                    $($name => ($pattern, $($callback)?)),+
                };

                pub fn offsets(view: PeView<'_>) -> BTreeMap<String, Rva> {
                    let mut map = BTreeMap::new();

                    for (&name, (pat, callback)) in &PATTERNS {
                        let mut save = vec![0; save_len(pat)];

                        if !view.scanner().finds_code(pat, &mut save) {
                            error!("outdated pattern: {}", name);

                            continue;
                        }

                        let rva = save[1];

                        map.insert(name.to_string(), rva);

                        if let Some(callback) = callback {
                            callback(&view, &mut map, rva);
                        }
                    }

                    for (name, value) in &map {
                        debug!(
                            "found offset: {} at {:#X} ({}.dll + {:#X})",
                            name,
                            *value as u64 + view.optional_header().ImageBase,
                            stringify!($module),
                            value
                        );
                    }

                    map
                }
            }
        )+
    };
}

pattern_map! {
    client => {
        "dwEntityList" => pattern!("488b0d${'} 48897c24? 8bfac1eb") => None,
        "dwViewMatrix" => pattern!("488d0d${'} 48c1e006") => None,
    }
}

pub fn offsets() -> Result<OffsetMap> {
    let mut map = BTreeMap::new();

    let modules: [(&str, fn(PeView) -> BTreeMap<String, u32>); 1] = [
        ("client.dll", client::offsets)
    ];
    let pid = secure_process_memory::return_pid("deadlock.exe").unwrap();
    let mut process = secure_process_memory::Process::new(pid).unwrap();
    let mut found = false;
    for (module_name, offsets) in &modules {
        if let Some(module) = dbg!(process.get_module_base_address(module_name))?{
            found = true;
            let buf = memory::memory::read_sized(module as usize,0x2ECDD00).unwrap(); //FIXME: Add module sizing to secure_process_memory

            let view = PeView::from_bytes(&buf)?;

            map.insert(module_name.to_string(), offsets(view));
        }
    }
    if found{
        Ok(map)
    } else {
        Err(anyhow::Error::msg("failed to find"))
    }
}

static CLIENT_BASE: LazyLock<usize> = LazyLock::new(|| {
    let pid = secure_process_memory::return_pid("deadlock.exe").unwrap();
    let process = secure_process_memory::Process::new(pid).unwrap();
    process.get_module_base_address("client.dll").unwrap().unwrap() as usize
});

static OFFSETS: OnceLock<[usize;2]> = OnceLock::new();


#[logic_system(name = "esp")]
fn esp(
    app: &mut App<AppData>,
    ui: &newoverlay::imgui::Ui,
    draw_list: &newoverlay::imgui::DrawListMut,
) {
    //dbg!("called every tick");
    let offsets = OFFSETS.get_or_init(|| {
        let offsets = offsets().unwrap();
        let val = offsets.get("client.dll").unwrap();
        [val.get("dwEntityList").unwrap().clone() as usize,val.get("dwViewMatrix").unwrap().clone() as usize]
    });
    let client_base: usize = *CLIENT_BASE;
    let dw_viewmatrix = read::<Matrix>(client_base + offsets[1]).unwrap();
    let matrix = Matrix::transpose(dw_viewmatrix);
    let viewport = Matrix::get_viewport(
        (0, 0),
        (
            app.window_info.size.0 as i32,
            app.window_info.size.1 as i32,
        ),
    ); //pure unadulterated retardium
    let viewmatrix = matrix * viewport;
    app.debug_text(format!("{:#?}",viewmatrix));
}

fn cache_players(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    let state = ctx.state();
    let offsets = OFFSETS.get_or_init(|| {
        let offsets = offsets().unwrap();
        let val = offsets.get("client.dll").unwrap();
        [val.get("dwEntityList").unwrap().clone() as usize,val.get("dwViewMatrix").unwrap().clone() as usize]
    });
    let client_base: usize = *CLIENT_BASE;
    let entity_list = read::<usize>(client_base + offsets[0]).unwrap();
    for i in 0..20 {
        let ent_entry = skip_err!(read::<usize>(
            entity_list + 8 * ((i & 0x7FFF) >> 9) + 16
        ));
        if ent_entry == 0 {
            continue;
        }
        dbg!(ent_entry);
        let controller = skip_err!(read::<usize>(ent_entry + 0x78 * (i & 0x1ff)));
        dbg!(controller);
        dbg!(read::<usize>(controller + 0x8F0 + 0x1c));
        let pawn_handle = skip_err!(read::<usize>(controller + 0x8ac));
        let entry = skip_err!(read::<usize>(
            entity_list + 0x8 * ((pawn_handle & 0x7FFF) >> 9) + 16
        ));
        let pawn = skip_err!(read::<usize>(entry + 0x78 * (pawn_handle & 0x1ff)));
        let scene_node = skip_err!(read::<usize>(pawn + 0x330));
        let pos = skip_err!(read::<Vector3>(scene_node + 0xc8));
        dbg!(i);
        dbg!(scene_node);
        dbg!(pos);
    }
    ThreadFlow::Continue
}


fn main() {
    newbase::init::custom_builder(AppData::default(),"deadlock.exe",Some(1422450))
        .expect("Failed to initialize runtime")
        .with_logic(Esp)
        .with_config_schema_str(SCHEMA_TOML, "config.toml")
        // .with_thread(
        //     "players",
        //     |x| cache_players(x)
        // )
        .run();
}
