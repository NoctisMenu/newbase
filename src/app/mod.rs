use anyhow::Result;
use windowing::WindowInfo;

mod gui;
mod logic;
mod overlay;
pub mod config_system;
mod macros;
mod threads;

use crate::{
    Animation, DoubleBuffer, Player,
    widgets::{MenuButton, SearchBar, Toasts},
};
use windows::Win32::Foundation::HWND;


use egui::{Pos2, Vec2, mutex::Mutex};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicI64},
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[derive(PartialEq)]
pub struct FloatingPoint {
    pub pos: Pos2,
    pub velocity: Vec2,
}
pub struct App {
    //internal details
    pub pid: u32,
    pub game_pid: u32,
    pub visible: bool,
    pub streamproofed: bool,
    pub visible_animation: Animation,
    pub show_time: std::time::Instant,
    pub init: bool,
    pub exit: bool,
    pub debug: String,
    pub game_window: windowing::Window,
    pub window_info: windowing::WindowInfo,
    pub time_remaining: Arc<AtomicI64>,
    pub device_state: device_query::DeviceState,
    join_handles: HashMap<String, JoinHandle<()>>,
    threads_performance: HashMap<String, Arc<Mutex<Duration>>>,

    //menu details
    pub frametime: Duration,
    pub frame_samples: Vec<Duration>,
    pub last_fps_update: Instant,
    pub averaged_fps: f32,
    pub true_frametime: Duration,
    pub true_frame_samples: Vec<Duration>,
    pub averaged_true_fps: f32,
    pub config_store: Arc<parking_lot::RwLock<config_system::ConfigStore>>,
    pub tab: MenuTab,
    pub aim_button: MenuButton,
    pub esp_button: MenuButton,
    pub exploits_button: MenuButton,
    pub misc_button: MenuButton,
    pub toasts: Toasts,

    //game details
    pub player_buffer: Arc<DoubleBuffer<Player>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            pid: 0,
            game_pid: 0,
            visible: true,
            streamproofed: false,
            visible_animation: Animation::new(std::time::Duration::from_millis(1500), None),
            show_time: std::time::Instant::now(),
            game_window: HWND::default(),
            window_info: WindowInfo {
                pos: (0, 0),
                size: (0, 0),
            },
            init: false,
            exit: false,
            time_remaining: Arc::new(AtomicI64::default()),
            device_state: device_query::DeviceState::new(),
            frametime: Duration::from_secs(1),
            frame_samples: Vec::new(),
            last_fps_update: Instant::now(),
            averaged_fps: 0.0,
            true_frametime: Duration::from_secs(1),
            true_frame_samples: Vec::new(),
            averaged_true_fps: 0.0,
            aim_button: MenuButton::new(None),
            esp_button: MenuButton::new(None),
            exploits_button: MenuButton::new(None),
            misc_button: MenuButton::new(None),
            toasts: Toasts::new(),

            config_store: Arc::new(parking_lot::RwLock::new(
                config_system::ConfigStore::load_with_fallback(),
            )),
            debug: String::new(),
            join_handles: HashMap::new(),
            threads_performance: HashMap::new(),
            tab: MenuTab::Aim,


            player_buffer: Arc::new(DoubleBuffer::<Player>::new(100)),
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, PartialOrd)]
pub enum MenuTab {
    #[default]
    Aim,
    Esp,
    Exploits,
    Misc,
}

impl App {
    pub fn start(
        game_pid: u32,
        game_window: windowing::Window,
        time_remaining: Arc<AtomicI64>,
    ) {
        let pid = std::process::id();
        let mut app = App {
            pid,
            game_pid,
            game_window,
            time_remaining,
            ..Default::default()
        };
        app.spawn_all_threads();
        egui_overlay::start(app)
    }

    pub fn threads_status(&self) -> Vec<(String, f32)> {
        let mut ret = Vec::new();
        for handle in &self.join_handles {
            ret.push((
                handle.0.clone(),
                self.threads_performance
                    .get(handle.0)
                    .unwrap()
                    .lock()
                    .as_secs_f32(),
            ));
        }
        ret
    }
    /// returns whether or not all threads are currently running, useful for terminating if any threads have stopped
    pub fn any_thread(&self) -> bool {
        for handle in &self.join_handles {
            if handle.1.is_finished() {
                return false;
            }
        }
        true
    }
    pub fn nthread<F>(&mut self, thread_name: impl ToString, mut f: F) -> Result<(), String>
    where
        F: FnMut() + Send + 'static,
    {
        if self.join_handles.contains_key(&thread_name.to_string()) {
            log::info!("Thread {} already exists!", thread_name.to_string());
            return Err("A thread already exists with that name!".to_string());
        }
        let thread_perf = self
            .threads_performance
            .entry(thread_name.to_string())
            .or_insert(Arc::new(Mutex::new(Duration::from_millis(0))))
            .clone();
        let handle = std::thread::Builder::new()
            .spawn(move || {
                loop {
                    #[cfg(debug_assertions)]
                    let start = Instant::now();
                    f();
                    //retarded
                    #[cfg(debug_assertions)]
                    {
                        *thread_perf.lock() = start.elapsed();
                    }
                }
            })
            .map_err(|err| format!("{}", err))?;
        self.join_handles
            .entry(thread_name.to_string())
            .or_insert(handle);
        Ok(())
    }
}
