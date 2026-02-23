use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mlua::{Function, Lua, Result as LuaResult, Table, Value};
use newbase::{ThreadCtx, ThreadFlow, read};

use crate::models::EntityType;
use crate::offsets::{client_base, resolved_offsets};
use crate::player::{Ability, AppData, Entity, Player};

const SCRIPT_DIR: &str = "scripts";
const SCRIPT_EXT: &str = "lua";
const DISCOVERY_INTERVAL: Duration = Duration::from_millis(750);
const SCRIPT_TICK_INTERVAL: Duration = Duration::from_millis(5);
const DEFAULT_CSTRING_LEN: usize = 96;
const MAX_CSTRING_LEN: usize = 1024;
const MAX_BYTE_READ_LEN: usize = 4096;

thread_local! {
    static SCRIPT_MANAGER: RefCell<ScriptManager> = RefCell::new(ScriptManager::new());
}

pub fn run(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    thread::sleep(SCRIPT_TICK_INTERVAL);

    let state = ctx.state();
    let players = state.player_buf.read().to_vec();
    let entities = state.entity_buf.read().to_vec();

    SCRIPT_MANAGER.with(|manager| manager.borrow_mut().tick(&players, &entities));

    ThreadFlow::Continue
}

struct ScriptManager {
    scripts: HashMap<PathBuf, ScriptInstance>,
    failed_loads: HashMap<PathBuf, SystemTime>,
    last_discovery: Instant,
}

impl ScriptManager {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            scripts: HashMap::new(),
            failed_loads: HashMap::new(),
            last_discovery: now.checked_sub(DISCOVERY_INTERVAL).unwrap_or(now),
        }
    }

    fn tick(&mut self, players: &[Player], entities: &[Entity]) {
        if self.last_discovery.elapsed() >= DISCOVERY_INTERVAL {
            self.refresh_scripts();
            self.last_discovery = Instant::now();
        }

        for script in self.scripts.values_mut() {
            script.tick(players, entities);
        }
    }

    fn refresh_scripts(&mut self) {
        let script_dir = Path::new(SCRIPT_DIR);
        if !script_dir.exists() {
            if let Err(error) = fs::create_dir_all(script_dir) {
                log::error!(
                    "failed to create script directory '{}': {}",
                    script_dir.display(),
                    error
                );
                return;
            }
            log::info!("created script directory '{}'", script_dir.display());
        }

        let discovered = discover_scripts(script_dir);
        let discovered_paths: HashSet<PathBuf> = discovered.keys().cloned().collect();

        // unload deleted scripts
        let loaded_paths: Vec<PathBuf> = self.scripts.keys().cloned().collect();
        for loaded_path in loaded_paths {
            if discovered_paths.contains(&loaded_path) {
                continue;
            }

            if let Some(old_script) = self.scripts.remove(&loaded_path) {
                if let Err(error) = old_script.call_on_unload() {
                    log::error!(
                        "[lua:{}] on_unload failed: {}",
                        script_label(&loaded_path),
                        error
                    );
                }
                log::info!("unloaded lua script '{}'", loaded_path.display());
            }
        }

        self.failed_loads
            .retain(|path, _| discovered_paths.contains(path));

        for (path, modified) in discovered {
            if let Some(existing) = self.scripts.get(&path)
                && existing.modified == modified
            {
                continue;
            }

            if let Some(existing_failed_mtime) = self.failed_loads.get(&path)
                && *existing_failed_mtime == modified
            {
                continue;
            }

            if self.scripts.contains_key(&path) {
                self.reload_script(path, modified);
            } else {
                self.load_script(path, modified);
            }
        }
    }

    fn load_script(&mut self, path: PathBuf, modified: SystemTime) {
        match ScriptInstance::load(&path, modified) {
            Ok(script) => {
                if let Err(error) = script.call_on_load() {
                    log::error!("[lua:{}] on_load failed: {}", script.label, error);
                }
                self.failed_loads.remove(&path);
                self.scripts.insert(path.clone(), script);
                log::info!("loaded lua script '{}'", path.display());
            }
            Err(error) => {
                self.failed_loads.insert(path.clone(), modified);
                log::error!("failed to load lua script '{}': {}", path.display(), error);
            }
        }
    }

    fn reload_script(&mut self, path: PathBuf, modified: SystemTime) {
        let Some(old_script) = self.scripts.remove(&path) else {
            self.load_script(path, modified);
            return;
        };

        match ScriptInstance::load(&path, modified) {
            Ok(script) => {
                if let Err(error) = old_script.call_on_unload() {
                    log::error!("[lua:{}] on_unload failed: {}", old_script.label, error);
                }
                if let Err(error) = script.call_on_load() {
                    log::error!("[lua:{}] on_load failed: {}", script.label, error);
                }
                self.failed_loads.remove(&path);
                self.scripts.insert(path.clone(), script);
                log::info!("reloaded lua script '{}'", path.display());
            }
            Err(error) => {
                self.failed_loads.insert(path.clone(), modified);
                log::error!(
                    "failed to reload lua script '{}': {} (keeping previous runtime)",
                    path.display(),
                    error
                );
                self.scripts.insert(path, old_script);
            }
        }
    }
}

struct ScriptInstance {
    path: PathBuf,
    label: String,
    modified: SystemTime,
    lua: Lua,
    last_tick_error: Option<String>,
}

impl ScriptInstance {
    fn load(path: &Path, modified: SystemTime) -> Result<Self, String> {
        let source = fs::read_to_string(path)
            .map_err(|error| format!("failed to read '{}': {}", path.display(), error))?;

        let label = script_label(path);
        let lua = Lua::new();
        register_lua_api(&lua, &label)
            .map_err(|error| format!("failed to register lua api: {}", error))?;

        lua.load(&source)
            .set_name(path.display().to_string())
            .exec()
            .map_err(|error| format!("execution failed: {}", error))?;

        Ok(Self {
            path: path.to_path_buf(),
            label,
            modified,
            lua,
            last_tick_error: None,
        })
    }

    fn tick(&mut self, players: &[Player], entities: &[Entity]) {
        match self.call_on_tick(players, entities) {
            Ok(()) => {
                self.last_tick_error = None;
            }
            Err(error) => {
                let msg = error.to_string();
                if self.last_tick_error.as_deref() != Some(msg.as_str()) {
                    log::error!("[lua:{}] on_tick failed: {}", self.label, msg);
                    self.last_tick_error = Some(msg);
                }
            }
        }
    }

    fn call_on_load(&self) -> LuaResult<()> {
        let ctx = build_tick_context(&self.lua, &[], &[])?;
        self.call_optional_function("on_load", ctx)
    }

    fn call_on_unload(&self) -> LuaResult<()> {
        self.call_optional_function("on_unload", ())
    }

    fn call_on_tick(&self, players: &[Player], entities: &[Entity]) -> LuaResult<()> {
        let ctx = build_tick_context(&self.lua, players, entities)?;
        self.call_optional_function("on_tick", ctx)
    }

    fn call_optional_function<A>(&self, callback: &str, args: A) -> LuaResult<()>
    where
        A: mlua::IntoLuaMulti,
    {
        let globals = self.lua.globals();
        let function: Option<Function> = globals.get(callback)?;
        if let Some(function) = function {
            function.call::<()>(args)?;
        }
        Ok(())
    }
}

fn discover_scripts(script_dir: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut discovered = HashMap::new();
    let Ok(entries) = fs::read_dir(script_dir) else {
        return discovered;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_lua_script(&path) {
            continue;
        }

        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .unwrap_or(UNIX_EPOCH);
        discovered.insert(path, modified);
    }

    discovered
}

fn is_lua_script(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case(SCRIPT_EXT))
        .unwrap_or(false)
}

fn script_label(path: &Path) -> String {
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        name.to_string()
    } else {
        path.display().to_string()
    }
}

fn register_lua_api(lua: &Lua, script_label: &str) -> LuaResult<()> {
    let globals = lua.globals();

    // Limit built-in capabilities; scripts can still use pure Lua stdlib.
    globals.set("os", Value::Nil)?;
    globals.set("io", Value::Nil)?;
    globals.set("package", Value::Nil)?;
    globals.set("debug", Value::Nil)?;
    globals.set("dofile", Value::Nil)?;
    globals.set("loadfile", Value::Nil)?;
    globals.set("require", Value::Nil)?;

    let read_api = build_read_api(lua)?;
    globals.set("read", read_api)?;

    let process = lua.create_table()?;
    process.set(
        "client_base",
        lua.create_function(|_, ()| Ok(client_base()))?,
    )?;
    process.set(
        "offsets",
        lua.create_function(|lua, ()| {
            let resolved = resolved_offsets();
            let table = lua.create_table()?;
            table.set("entity_list", resolved[0])?;
            table.set("view_matrix", resolved[1])?;
            Ok(table)
        })?,
    )?;
    globals.set("process", process)?;

    let console = lua.create_table()?;
    let info_label = script_label.to_string();
    let warn_label = script_label.to_string();
    let error_label = script_label.to_string();

    console.set(
        "info",
        lua.create_function(move |_, msg: String| {
            log::info!("[lua:{}] {}", info_label, msg);
            Ok(())
        })?,
    )?;
    console.set(
        "warn",
        lua.create_function(move |_, msg: String| {
            log::warn!("[lua:{}] {}", warn_label, msg);
            Ok(())
        })?,
    )?;
    console.set(
        "error",
        lua.create_function(move |_, msg: String| {
            log::error!("[lua:{}] {}", error_label, msg);
            Ok(())
        })?,
    )?;
    globals.set("console", console)?;

    Ok(())
}

fn build_read_api(lua: &Lua) -> LuaResult<Table> {
    let read_table = lua.create_table()?;

    read_table.set(
        "u8",
        lua.create_function(|_, addr: usize| Ok(read::<u8>(addr).ok()))?,
    )?;
    read_table.set(
        "u16",
        lua.create_function(|_, addr: usize| Ok(read::<u16>(addr).ok()))?,
    )?;
    read_table.set(
        "u32",
        lua.create_function(|_, addr: usize| Ok(read::<u32>(addr).ok()))?,
    )?;
    read_table.set(
        "u64",
        lua.create_function(|_, addr: usize| Ok(read::<u64>(addr).ok()))?,
    )?;
    read_table.set(
        "i8",
        lua.create_function(|_, addr: usize| Ok(read::<i8>(addr).ok()))?,
    )?;
    read_table.set(
        "i16",
        lua.create_function(|_, addr: usize| Ok(read::<i16>(addr).ok()))?,
    )?;
    read_table.set(
        "i32",
        lua.create_function(|_, addr: usize| Ok(read::<i32>(addr).ok()))?,
    )?;
    read_table.set(
        "i64",
        lua.create_function(|_, addr: usize| Ok(read::<i64>(addr).ok()))?,
    )?;
    read_table.set(
        "f32",
        lua.create_function(|_, addr: usize| Ok(read::<f32>(addr).ok()))?,
    )?;
    read_table.set(
        "f64",
        lua.create_function(|_, addr: usize| Ok(read::<f64>(addr).ok()))?,
    )?;
    read_table.set(
        "bool",
        lua.create_function(|_, addr: usize| Ok(read::<bool>(addr).ok()))?,
    )?;
    read_table.set(
        "ptr",
        lua.create_function(|_, addr: usize| Ok(read::<usize>(addr).ok()))?,
    )?;

    read_table.set(
        "cstr",
        lua.create_function(|_, (addr, max_len): (usize, Option<usize>)| {
            let max_len = max_len
                .unwrap_or(DEFAULT_CSTRING_LEN)
                .clamp(1, MAX_CSTRING_LEN);
            Ok(read_c_string(addr, max_len))
        })?,
    )?;

    read_table.set(
        "bytes",
        lua.create_function(|lua, (addr, len): (usize, usize)| {
            let len = len.min(MAX_BYTE_READ_LEN);
            if len == 0 {
                let empty = lua.create_string(&[] as &[u8])?;
                return Ok(Value::String(empty));
            }
            let mut out = Vec::with_capacity(len);
            for offset in 0..len {
                let Ok(byte) = read::<u8>(addr + offset) else {
                    return Ok(Value::Nil);
                };
                out.push(byte);
            }

            let bytes = lua.create_string(&out)?;
            Ok(Value::String(bytes))
        })?,
    )?;

    read_table.set(
        "ptr_chain",
        lua.create_function(|_, (base, offsets): (usize, Vec<usize>)| {
            if base == 0 {
                return Ok(None::<usize>);
            }

            let mut current = base;
            for offset in offsets {
                let addr = current.saturating_add(offset);
                let Ok(next) = read::<usize>(addr) else {
                    return Ok(None::<usize>);
                };
                if next == 0 {
                    return Ok(None::<usize>);
                }
                current = next;
            }

            Ok(Some(current))
        })?,
    )?;

    Ok(read_table)
}

fn read_c_string(addr: usize, max_len: usize) -> Option<String> {
    if addr == 0 || max_len == 0 {
        return None;
    }

    let mut out = Vec::with_capacity(max_len);
    let mut terminated = false;
    for offset in 0..max_len {
        let byte = read::<u8>(addr + offset).ok()?;
        if byte == 0 {
            terminated = true;
            break;
        }
        out.push(byte);
    }

    if !terminated || out.is_empty() {
        return None;
    }

    Some(String::from_utf8_lossy(&out).into_owned())
}

fn build_tick_context(lua: &Lua, players: &[Player], entities: &[Entity]) -> LuaResult<Table> {
    let ctx = lua.create_table()?;
    ctx.set("timestamp_ms", now_unix_ms())?;

    let players_table = lua.create_table()?;
    let mut local_player: Option<Table> = None;
    for (idx, player) in players.iter().enumerate() {
        let player_table = player_to_lua(lua, player, idx)?;
        if player.is_local {
            local_player = Some(player_table.clone());
        }
        players_table.set(idx + 1, player_table)?;
    }
    ctx.set("players", players_table)?;
    ctx.set("player_count", players.len())?;
    ctx.set("local_player", local_player)?;

    let entities_table = lua.create_table()?;
    for (idx, entity) in entities.iter().enumerate() {
        entities_table.set(idx + 1, entity_to_lua(lua, entity)?)?;
    }
    ctx.set("entities", entities_table)?;
    ctx.set("entity_count", entities.len())?;

    Ok(ctx)
}

fn player_to_lua(lua: &Lua, player: &Player, index: usize) -> LuaResult<Table> {
    let table = lua.create_table()?;

    table.set("index", index)?;
    table.set(
        "pos",
        vec3_to_lua(lua, player.pos.x, player.pos.y, player.pos.z)?,
    )?;
    table.set("alive", player.alive)?;
    table.set("health", player.health)?;
    table.set("max_health", player.max_health)?;
    table.set("is_local", player.is_local)?;
    table.set("team_id", player.team_id)?;
    table.set("ult_cd", player.ult_cd)?;
    table.set("hero", player.hero.to_string())?;
    table.set("hero_id", player.hero as i32)?;

    let bones = lua.create_table()?;
    for (bone_idx, bone) in player.bones.iter().enumerate() {
        bones.set(
            bone_idx + 1,
            vec3_to_lua(lua, bone.x as f32, bone.y as f32, bone.z as f32)?,
        )?;
    }
    table.set("bones", bones)?;

    let skeleton_links = lua.create_table()?;
    for (limb_idx, limb) in player.skeleton_links.iter().enumerate() {
        let limb_links = lua.create_table()?;
        for (link_idx, bone_idx) in limb.iter().enumerate() {
            limb_links.set(link_idx + 1, *bone_idx)?;
        }
        skeleton_links.set(limb_idx + 1, limb_links)?;
    }
    table.set("skeleton_links", skeleton_links)?;

    let abilities = lua.create_table()?;
    for (ability_idx, ability) in player.abilities.iter().enumerate() {
        abilities.set(ability_idx + 1, ability_to_lua(lua, ability)?)?;
    }
    table.set("abilities", abilities)?;

    Ok(table)
}

fn ability_to_lua(lua: &Lua, ability: &Ability) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set("slot", format!("{:?}", ability.slot))?;
    table.set("slot_id", ability.slot as u16)?;
    table.set("cooling_down", ability.cooling_down)?;
    table.set("channeling", ability.channeling)?;
    table.set("cooldown_start", ability.cooldown_start)?;
    table.set("cooldown_end", ability.cooldown_end)?;
    table.set("data_ptr", ability.data_ptr)?;
    Ok(table)
}

fn entity_to_lua(lua: &Lua, entity: &Entity) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set("name", entity.name.clone())?;
    table.set("type", entity_type_name(entity.e_type))?;
    table.set("visible", entity.visible)?;
    table.set("attackable", entity.attackable)?;
    table.set(
        "pos",
        vec3_to_lua(lua, entity.pos.x, entity.pos.y, entity.pos.z)?,
    )?;
    Ok(table)
}

fn vec3_to_lua(lua: &Lua, x: f32, y: f32, z: f32) -> LuaResult<Table> {
    let table = lua.create_table()?;
    table.set("x", x)?;
    table.set("y", y)?;
    table.set("z", z)?;
    Ok(table)
}

fn entity_type_name(entity_type: EntityType) -> &'static str {
    match entity_type {
        EntityType::Soul => "soul",
        EntityType::Creep => "creep",
    }
}

fn now_unix_ms() -> i64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    duration.as_millis() as i64
}
