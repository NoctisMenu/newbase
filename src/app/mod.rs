use anyhow::Result;
use windowing::WindowInfo;

use std::sync::Mutex;

pub mod config_system;
mod gui;
mod logic;

use crate::{DoubleBuffer, Player};
use windows::Win32::Foundation::HWND;

use newoverlay::Overlay;
use newoverlay::imgui::ImColor32;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicI64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

// Thread control flow
pub enum ThreadFlow {
    Continue,
    Stop,
}

// Context passed to threads
#[derive(Clone)]
pub struct ThreadCtx<S> {
    shutdown: Arc<AtomicBool>,
    state: Arc<S>,
}

impl<S> ThreadCtx<S> {
    pub fn should_stop(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn state(&self) -> &Arc<S> {
        &self.state
    }
}

// Logic system trait for frame-based logic
pub trait LogicSystem<S>: Send {
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn tick(&mut self, app: &mut App<S>, ui: &newoverlay::imgui::Ui);
}

#[derive(PartialEq)]
pub struct FloatingPoint {
    pub pos: [f32; 2],
    pub velocity: [f32; 2],
}

pub struct App<S> {
    pub pid: u32,
    pub game_pid: u32,
    pub visible: bool,
    pub streamproofed: bool,
    pub show_time: std::time::Instant,
    pub init: bool,
    pub exit: bool,
    pub debug: String,
    pub game_window: windowing::Window,
    pub window_info: windowing::WindowInfo,
    pub time_remaining: Arc<AtomicI64>,
    pub device_state: device_query::DeviceState,

    // Thread management
    shutdown: Arc<AtomicBool>,
    join_handles: HashMap<String, JoinHandle<()>>,
    threads_performance: HashMap<String, Arc<Mutex<Duration>>>,

    // User state
    pub state: Arc<S>,

    // Logic systems
    logic_systems: Vec<Box<dyn LogicSystem<S>>>,

    // FPS tracking
    pub frametime: Duration,
    pub frame_samples: Vec<Duration>,
    pub last_fps_update: Instant,
    pub averaged_fps: f32,
    pub true_frametime: Duration,
    pub true_frame_samples: Vec<Duration>,
    pub averaged_true_fps: f32,

    pub config_store: Arc<parking_lot::RwLock<config_system::ConfigStore>>,
}

// Builder pattern for easier setup
pub struct AppBuilder<S> {
    app: App<S>,
}

impl<S: Send + Sync + 'static> AppBuilder<S> {
    pub fn new(
        game_pid: u32,
        game_window: windowing::Window,
        time_remaining: Arc<AtomicI64>,
        state: S,
    ) -> Self {
        let pid = std::process::id();
        let app = App {
            pid,
            game_pid,
            game_window,
            time_remaining: time_remaining.clone(),
            state: Arc::new(state),
            shutdown: Arc::new(AtomicBool::new(false)),
            visible: true,
            streamproofed: false,
            show_time: Instant::now(),
            window_info: WindowInfo {
                pos: (0, 0),
                size: (0, 0),
            },
            init: false,
            exit: false,
            device_state: device_query::DeviceState::new(),
            frametime: Duration::from_secs(1),
            frame_samples: Vec::new(),
            last_fps_update: Instant::now(),
            averaged_fps: 0.0,
            true_frametime: Duration::from_secs(1),
            true_frame_samples: Vec::new(),
            averaged_true_fps: 0.0,
            config_store: Arc::new(parking_lot::RwLock::new(
                config_system::ConfigStore::load_with_fallback(),
            )),
            debug: String::new(),
            join_handles: HashMap::new(),
            threads_performance: HashMap::new(),
            logic_systems: Vec::new(),
        };
        Self { app }
    }

    /// Add a thread that runs continuously
    pub fn with_thread<F>(mut self, name: impl Into<String>, task: F) -> Self
    where
        F: FnMut(&ThreadCtx<S>) -> ThreadFlow + Send + 'static,
    {
        let name = name.into();
        if let Err(e) = self.app.spawn_thread(name.clone(), task) {
            log::error!("Failed to register thread '{}': {}", name, e);
        }
        self
    }

    /// Add a logic system that runs each frame
    pub fn with_logic<L>(mut self, system: L) -> Self
    where
        L: LogicSystem<S> + 'static,
    {
        self.app.add_logic(system);
        self
    }

    /// Register default threads (from threads module)
    pub fn with_default_threads(mut self) -> Self {
        self.app.spawn_all_threads();
        self
    }

    pub fn build(self) -> App<S> {
        self.app
    }

    /// Build and immediately run
    pub fn run(mut self) {
        self.app.run();
    }
}

impl<S: Send + Sync + 'static> App<S> {
    /// Create a builder for configuring the app
    pub fn builder(
        game_pid: u32,
        game_window: windowing::Window,
        time_remaining: Arc<AtomicI64>,
        state: S,
    ) -> AppBuilder<S> {
        AppBuilder::new(game_pid, game_window, time_remaining, state)
    }

    // === Logic System Management ===

    /// Add a logic system at runtime
    pub fn add_logic<L>(&mut self, system: L)
    where
        L: LogicSystem<S> + 'static,
    {
        self.logic_systems.push(Box::new(system));
    }

    /// Run all logic systems (called each frame)
    pub fn tick_logic(&mut self, ui: &newoverlay::imgui::Ui) {
        for system in &mut self.logic_systems {
            system.tick(self, ui);
        }
    }

    // === Thread Management ===

    /// Spawn a new thread at runtime
    pub fn spawn_thread<F>(
        &mut self,
        thread_name: impl Into<String>,
        mut task: F,
    ) -> Result<(), String>
    where
        F: FnMut(&ThreadCtx<S>) -> ThreadFlow + Send + 'static,
    {
        let thread_name = thread_name.into();

        if self.join_handles.contains_key(&thread_name) {
            log::info!("Thread {} already exists!", thread_name);
            return Err("A thread already exists with that name!".to_string());
        }

        let thread_perf = self
            .threads_performance
            .entry(thread_name.clone())
            .or_insert(Arc::new(Mutex::new(Duration::from_millis(0))))
            .clone();

        let ctx = ThreadCtx {
            shutdown: self.shutdown.clone(),
            state: self.state.clone(),
        };

        let handle = std::thread::Builder::new()
            .name(thread_name.clone())
            .spawn(move || {
                while !ctx.should_stop() {
                    #[cfg(debug_assertions)]
                    let start = Instant::now();

                    let flow = task(&ctx);

                    #[cfg(debug_assertions)]
                    {
                        *thread_perf.lock().unwrap() = start.elapsed();
                    }

                    if matches!(flow, ThreadFlow::Stop) {
                        break;
                    }
                }
            })
            .map_err(|err| err.to_string())?;

        self.join_handles.insert(thread_name, handle);
        Ok(())
    }

    /// Get a handle for thread-safe app access
    pub fn handle(&self) -> ThreadCtx<S> {
        ThreadCtx {
            shutdown: self.shutdown.clone(),
            state: self.state.clone(),
        }
    }

    /// Request shutdown of all threads
    pub fn request_shutdown(&mut self) {
        self.exit = true;
        self.shutdown.store(true, Ordering::Release);
    }

    /// Stop all running threads
    pub fn stop_all_threads(&mut self) {
        self.shutdown.store(true, Ordering::Release);

        for (name, handle) in std::mem::take(&mut self.join_handles) {
            if handle.join().is_err() {
                log::error!("Thread '{}' panicked during shutdown", name);
            }
        }
    }

    pub fn threads_status(&self) -> Vec<(String, f32)> {
        let mut ret = Vec::new();
        for handle in &self.join_handles {
            ret.push((
                handle.0.clone(),
                self.threads_performance
                    .get(handle.0)
                    .map(|perf| {
                        let perf = perf.lock().unwrap();
                        if perf.as_millis() == 0 {
                            0.0
                        } else {
                            (1000.0 / perf.as_millis() as f32).min(9999.0)
                        }
                    })
                    .unwrap_or(0.0),
            ));
        }
        ret
    }

    /// Returns whether or not all threads are currently running
    pub fn any_thread(&self) -> bool {
        for handle in &self.join_handles {
            if handle.1.is_finished() {
                return false;
            }
        }
        true
    }
}

impl<S> Drop for App<S> {
    fn drop(&mut self) {
        self.stop_all_threads();
    }
}

// For apps that don't need custom state
impl App<()> {
    pub fn start(game_pid: u32, game_window: windowing::Window, time_remaining: Arc<AtomicI64>) {
        App::builder(game_pid, game_window, time_remaining, ())
            .with_default_threads()
            .run();
    }
}
