use std::cell::RefCell;
use std::f64::consts::PI;
use std::time::{Duration, Instant};

use device_query::{DeviceQuery, DeviceState};
use newbase::{ThreadCtx, ThreadFlow, read};
use rand::Rng;
use unreal_types::ue5::FRotator;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
};

use crate::models::math::{Matrix, Vector3};
use crate::offsets::{client_base, resolved_offsets};
use crate::player::{AppData, Player};

const AIM_ENABLED: bool = true;
const HUMANIZE_AIM: bool = true;
const AIM_FOV: f64 = 500.0;
const AIM_SMOOTHING: f64 = 0.48;
const REACTION_MIN_MS: f64 = 120.0;
const REACTION_MAX_MS: f64 = 250.0;
const UPDATE_RATE_JITTER_MS: f64 = 3.0;
const BASE_UPDATE_MS: f64 = 15.0;
const SCREEN_WIDTH: f64 = 2560.0;
const SCREEN_HEIGHT: f64 = 1440.0;

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum Hitbox {
    Head,
    Chest,
    Pelvis,
}

const HITBOX: Hitbox = Hitbox::Head;

struct AimState {
    last_target_name: String,
    target_acquired_at: Option<Instant>,
    reaction_delay: Duration,
    mouse_accumulator_x: f64,
    mouse_accumulator_y: f64,
}

impl Default for AimState {
    fn default() -> Self {
        Self {
            last_target_name: String::new(),
            target_acquired_at: None,
            reaction_delay: Duration::from_millis(0),
            mouse_accumulator_x: 0.0,
            mouse_accumulator_y: 0.0,
        }
    }
}

thread_local! {
    static AIM_STATE: RefCell<AimState> = RefCell::new(AimState::default());
    static DEVICE_STATE: DeviceState = DeviceState::new();
}
fn sleep_with_jitter() {
    if HUMANIZE_AIM && UPDATE_RATE_JITTER_MS > 0.0 {
        let mut rng = rand::rng();
        let jitter = rng.random_range(-UPDATE_RATE_JITTER_MS..=UPDATE_RATE_JITTER_MS);
        let sleep_ms = (BASE_UPDATE_MS + jitter).max(1.0) as u64;
        std::thread::sleep(Duration::from_millis(sleep_ms));
    } else {
        std::thread::sleep(Duration::from_millis(BASE_UPDATE_MS as u64));
    }
}

fn pick_target_bone(player: &Player) -> Option<Vector3> {
    let spine = &player.skeleton_links[0];
    let idx = match HITBOX {
        Hitbox::Head => spine.last().copied(),
        Hitbox::Chest => spine.get(2).copied().or_else(|| spine.get(1).copied()),
        Hitbox::Pelvis => spine.first().copied(),
    }
    .or_else(|| {
        player
            .hero
            .get_head_bone()
            .and_then(|bone| usize::try_from(bone).ok())
    })
    .or_else(|| (!player.bones.is_empty()).then_some(0))?;

    let bone = *player.bones.get(idx)?;
    Some(Vector3 {
        x: bone.x as f32,
        y: bone.y as f32,
        z: bone.z as f32,
    })
}

fn should_react_delay(target_name: &str) -> bool {
    if !HUMANIZE_AIM {
        return false;
    }

    AIM_STATE.with(|state| {
        let mut state = state.borrow_mut();

        if target_name != state.last_target_name {
            let mut rng = rand::rng();
            let (min_delay, max_delay) = if REACTION_MIN_MS <= REACTION_MAX_MS {
                (REACTION_MIN_MS, REACTION_MAX_MS)
            } else {
                (REACTION_MAX_MS, REACTION_MIN_MS)
            };
            let delay_ms = rng.random_range(min_delay..=max_delay);
            state.reaction_delay = Duration::from_millis(delay_ms as u64);
            state.target_acquired_at = Some(Instant::now());
            state.last_target_name.clear();
            state.last_target_name.push_str(target_name);
            return true;
        }

        if let Some(acquired_time) = state.target_acquired_at {
            return acquired_time.elapsed() < state.reaction_delay;
        }

        false
    })
}

fn consume_smoothed_mouse(x: f64, y: f64) -> (i32, i32) {
    if !HUMANIZE_AIM {
        return (x.floor() as i32, y.floor() as i32);
    }

    AIM_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.mouse_accumulator_x += x;
        state.mouse_accumulator_y += y;

        let move_x = state.mouse_accumulator_x.trunc();
        let move_y = state.mouse_accumulator_y.trunc();

        state.mouse_accumulator_x -= move_x;
        state.mouse_accumulator_y -= move_y;

        (move_x as i32, move_y as i32)
    })
}

fn should_aim_now() -> bool {
    DEVICE_STATE.with(|device| {
        let buttons = device.get_mouse().button_pressed;
        let right = buttons.get(1).copied().unwrap_or(false);
        let middle = buttons.get(2).copied().unwrap_or(false);
        right || middle
    })
}

fn move_mouse(dx: i32, dy: i32) {
    if dx == 0 && dy == 0 {
        return;
    }

    unsafe {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn run(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    sleep_with_jitter();

    if !AIM_ENABLED {
        return ThreadFlow::Continue;
    }

    let players = ctx.state().player_buf.read();
    if players.is_empty() {
        return ThreadFlow::Continue;
    }

    let local_alive = players.iter().any(|player| player.is_local && player.health > 0);
    if !local_alive {
        return ThreadFlow::Continue;
    }

    let offsets = resolved_offsets();
    let base = client_base();
    let raw_matrix = match read::<Matrix>(base + offsets[1]) {
        Ok(matrix) => matrix,
        Err(_) => return ThreadFlow::Continue,
    };
    let matrix = Matrix::transpose(raw_matrix);
    let viewport = Matrix::get_viewport((0, 0), (SCREEN_WIDTH as i32, SCREEN_HEIGHT as i32));
    let view_matrix = matrix * viewport;

    let mut best_distance: Option<f64> = None;
    let mut best_target: Option<(f64, f64)> = None;
    let mut best_target_name = String::new();

    for (idx, player) in players.iter().enumerate() {
        if player.is_local || !player.alive || player.health <= 0 {
            continue;
        }

        let bone = match pick_target_bone(player) {
            Some(bone) => bone,
            None => continue,
        };

        let (screen_x, screen_y) = match view_matrix.transform(&bone) {
            Some((x, y)) => (x as f64, y as f64),
            None => continue,
        };

        let dx = SCREEN_WIDTH * 0.5 - screen_x;
        let dy = SCREEN_HEIGHT * 0.5 - screen_y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > AIM_FOV {
            continue;
        }

        if best_distance.is_none_or(|current| dist < current) {
            best_distance = Some(dist);
            best_target = Some((screen_x, screen_y));
            best_target_name = format!("{}-{}", player.hero.to_string(), idx);
        }
    }

    let Some((target_x, target_y)) = best_target else {
        return ThreadFlow::Continue;
    };

    if should_react_delay(&best_target_name) {
        return ThreadFlow::Continue;
    }

    let x = (target_x - SCREEN_WIDTH * 0.5) * AIM_SMOOTHING;
    let y = (target_y - SCREEN_HEIGHT * 0.5) * AIM_SMOOTHING;

    if x.abs() < 1.0 && y.abs() < 1.0 {
        return ThreadFlow::Continue;
    }

    let (move_x, move_y) = consume_smoothed_mouse(x, y);

    if should_aim_now() {
        move_mouse(move_x, move_y);
    }

    ThreadFlow::Continue
}
