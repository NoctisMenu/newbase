use std::cell::RefCell;
use std::f64::consts::PI;
use std::time::{Duration, Instant};

use device_query::{DeviceQuery, DeviceState};
use newbase::{ThreadCtx, ThreadFlow, read};
use rand::Rng;
use unreal_types::ue5::{FRotator, FVector};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
};

use crate::models::EntityType;
use crate::models::math::{Matrix, Vector3};
use crate::offsets::{client_base, resolved_offsets};
use crate::player::{AppData, Player};

const AIM_ENABLED: bool = true;
const HUMANIZE_AIM: bool = true;

const AIM_FOV: f64 = 150.0;
const AIM_SMOOTHING: f64 = 0.08;
const AIM_DISTANCE_ENABLED: bool = false;
const AIM_DISTANCE: f64 = 1000.0;

const REACTION_MIN_MS: f64 = 120.0;
const REACTION_MAX_MS: f64 = 250.0;
const UPDATE_RATE_JITTER_MS: f64 = 3.0;
const PREDICTION_SMOOTHING: f64 = 0.3;

const OVERSHOOT_CHANCE: f64 = 15.0;
const DYNAMIC_SMOOTHING: bool = true;
const MOVEMENT_THRESHOLD: f64 = 5.0;

const BASE_UPDATE_MS: f64 = 15.0;
const TARGET_LOST_RESET_MS: u64 = 350;
const CURSOR_LOCK_BYPASS_FRAMES: u32 = 12;
const SOUL_TARGET_ID_BASE: usize = 1_000_000;

const SCREEN_WIDTH: f64 = 2560.0;
const SCREEN_HEIGHT: f64 = 1440.0;
const SCREEN_CENTER_X: f64 = SCREEN_WIDTH * 0.5;
const SCREEN_CENTER_Y: f64 = SCREEN_HEIGHT * 0.5;

#[derive(Clone, Copy)]
enum TeamCheck {
    Enemies,
    Teammates,
    All,
}

const TEAM_CHECK: TeamCheck = TeamCheck::Enemies;

#[derive(Clone, Copy)]
enum Hitbox {
    Head,
    Neck,
    Chest,
    Pelvis,
}

const HITBOX: Hitbox = Hitbox::Head;

struct AimState {
    last_target_id: usize,
    last_target_seen_at: Option<Instant>,
    target_acquired_at: Option<Instant>,
    reaction_delay: Duration,
    mouse_accumulator_x: f64,
    mouse_accumulator_y: f64,
    last_predicted_pos: Option<FVector>,
    shake_phase_x: f64,
    shake_phase_y: f64,
    shake_time: f64,
    velocity_x: f64,
    velocity_y: f64,
    cursor_history: [(i32, i32); 4],
    history_index: usize,
    cursor_static_frames: u32,
    cursor_history_initialized: bool,
}

impl Default for AimState {
    fn default() -> Self {
        Self {
            last_target_id: 0,
            last_target_seen_at: None,
            target_acquired_at: None,
            reaction_delay: Duration::from_millis(0),
            mouse_accumulator_x: 0.0,
            mouse_accumulator_y: 0.0,
            last_predicted_pos: None,
            shake_phase_x: 0.0,
            shake_phase_y: 0.0,
            shake_time: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            cursor_history: [(0, 0); 4],
            history_index: 0,
            cursor_static_frames: 0,
            cursor_history_initialized: false,
        }
    }
}

thread_local! {
    static AIM_STATE: RefCell<AimState> = RefCell::new(AimState::default());
    static DEVICE_STATE: DeviceState = DeviceState::new();
}

fn reset_human_state(state: &mut AimState) {
    state.mouse_accumulator_x = 0.0;
    state.mouse_accumulator_y = 0.0;
    state.last_predicted_pos = None;
    state.velocity_x = 0.0;
    state.velocity_y = 0.0;
}

#[allow(dead_code)]
fn calculate_aim(from: &FVector, to: &FVector) -> FRotator {
    let delta = FVector {
        x: to.x - from.x,
        y: to.y - from.y,
        z: to.z - from.z,
    };
    let distance = (delta.x.powi(2) + delta.y.powi(2)).sqrt();

    FRotator {
        pitch: delta.z.atan2(distance) * 180.0 / PI,
        yaw: delta.y.atan2(delta.x) * 180.0 / PI,
        roll: 0.0,
    }
}

#[allow(dead_code)]
fn angular_difference(a: &FRotator, b: &FRotator) -> (f64, f64) {
    let mut pitch_diff = a.pitch - b.pitch;
    let mut yaw_diff = a.yaw - b.yaw;

    while pitch_diff > 180.0 {
        pitch_diff -= 360.0;
    }
    while pitch_diff < -180.0 {
        pitch_diff += 360.0;
    }
    while yaw_diff > 180.0 {
        yaw_diff -= 360.0;
    }
    while yaw_diff < -180.0 {
        yaw_diff += 360.0;
    }

    (pitch_diff, yaw_diff)
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

fn pick_target_bone(player: &Player) -> Option<FVector> {
    if player.bones.is_empty() {
        return None;
    }

    let spine = &player.skeleton_links[0];
    let index = match HITBOX {
        Hitbox::Head => spine
            .last()
            .copied()
            .or_else(|| player.bones.get(3).map(|_| 3)),
        Hitbox::Neck => spine
            .iter()
            .rev()
            .nth(1)
            .copied()
            .or_else(|| player.bones.get(2).map(|_| 2)),
        Hitbox::Chest => spine
            .get(2)
            .copied()
            .or_else(|| spine.get(1).copied())
            .or_else(|| player.bones.get(1).map(|_| 1)),
        Hitbox::Pelvis => spine
            .first()
            .copied()
            .or_else(|| player.bones.first().map(|_| 0)),
    }?;

    player.bones.get(index).copied()
}

fn world_to_screen(view_matrix: &Matrix, world: FVector) -> Option<(f64, f64)> {
    let world = Vector3 {
        x: world.x as f32,
        y: world.y as f32,
        z: world.z as f32,
    };
    view_matrix
        .transform(&world)
        .map(|(x, y)| (x as f64, y as f64))
}

fn read_mouse_state() -> ((i32, i32), bool) {
    DEVICE_STATE.with(|device| {
        let mouse = device.get_mouse();
        let buttons = mouse.button_pressed;
        let pressed =
            buttons.get(1).copied().unwrap_or(false) || buttons.get(2).copied().unwrap_or(false);
        (mouse.coords, pressed)
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

fn as_fvector(world: Vector3) -> FVector {
    FVector {
        x: world.x as f64,
        y: world.y as f64,
        z: world.z as f64,
    }
}

pub fn run(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    sleep_with_jitter();

    if !AIM_ENABLED {
        return ThreadFlow::Continue;
    }

    let state = ctx.state();
    let players = state.player_buf.read();
    let entities = state.entity_buf.read();
    if players.is_empty() {
        return ThreadFlow::Continue;
    }

    let local = match players
        .iter()
        .find(|p| p.is_local && p.alive && p.health > 0)
    {
        Some(local) => local,
        None => return ThreadFlow::Continue,
    };
    let local_team_id = local.team_id;
    let local_pos = as_fvector(local.pos);

    if HUMANIZE_AIM && MOVEMENT_THRESHOLD > 0.0 {
        let (cursor_now, _) = read_mouse_state();

        let skip_for_movement_gate = AIM_STATE.with(|state| {
            let mut state = state.borrow_mut();

            if !state.cursor_history_initialized {
                state.cursor_history.fill(cursor_now);
                state.cursor_history_initialized = true;
            }

            let mut total_movement = 0.0;
            for i in 0..3 {
                let prev_idx = (state.history_index + 4 - 3 + i) % 4;
                let curr_idx = (state.history_index + 4 - 3 + i + 1) % 4;
                let prev = state.cursor_history[prev_idx];
                let curr = state.cursor_history[curr_idx];
                let dx = (curr.0 - prev.0) as f64;
                let dy = (curr.1 - prev.1) as f64;
                total_movement += (dx * dx + dy * dy).sqrt();
            }

            state.history_index = (state.history_index + 1) % 4;
            let write_idx = state.history_index;
            state.cursor_history[write_idx] = cursor_now;

            if total_movement < MOVEMENT_THRESHOLD {
                state.cursor_static_frames = state.cursor_static_frames.saturating_add(1);
            } else {
                state.cursor_static_frames = 0;
            }

            total_movement < MOVEMENT_THRESHOLD
                && state.cursor_static_frames < CURSOR_LOCK_BYPASS_FRAMES
        });

        if skip_for_movement_gate {
            return ThreadFlow::Continue;
        }
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

    let mut best_angle_dist: Option<f64> = None;
    let mut best_target = None;
    let mut best_target_id: usize = 0;
    let mut best_raw_predicted = None;
    let mut soul_target_found = false;

    // Soul targeting takes absolute precedence over player targeting.
    for (idx, entity) in entities.iter().enumerate() {
        if !entity.visible || !matches!(entity.e_type, EntityType::Soul) {
            continue;
        }

        if !entity.attackable {
            continue;
        }

        let raw_predicted = as_fvector(entity.pos);
        let target_location = match world_to_screen(&view_matrix, raw_predicted) {
            Some(screen) => screen,
            None => continue,
        };

        if !target_location.0.is_finite() || !target_location.1.is_finite() {
            continue;
        }

        let angle_dist = ((SCREEN_CENTER_X - target_location.0).powi(2)
            + (SCREEN_CENTER_Y - target_location.1).powi(2))
        .sqrt();

        if !angle_dist.is_finite() || angle_dist > AIM_FOV {
            continue;
        }

        if best_target.is_none() || angle_dist < best_angle_dist.unwrap_or(f64::MAX) {
            best_target = Some(target_location);
            best_angle_dist = Some(angle_dist);
            best_target_id = SOUL_TARGET_ID_BASE + idx;
            best_raw_predicted = Some(raw_predicted);
            soul_target_found = true;
        }
    }

    if !soul_target_found {
        for (idx, player) in players.iter().enumerate() {
            if player.is_local || !player.alive || player.health <= 0 {
                continue;
            }

            match TEAM_CHECK {
                TeamCheck::Enemies if player.team_id == local_team_id => continue,
                TeamCheck::Teammates if player.team_id != local_team_id => continue,
                _ => {}
            }

            let bone_location = match pick_target_bone(player) {
                Some(bone) => bone,
                None => continue,
            };

            if AIM_DISTANCE_ENABLED {
                let bone_vec = Vector3 {
                    x: bone_location.x as f32,
                    y: bone_location.y as f32,
                    z: bone_location.z as f32,
                };
                let local_vec = Vector3 {
                    x: local_pos.x as f32,
                    y: local_pos.y as f32,
                    z: local_pos.z as f32,
                };
                if Vector3::distance(local_vec, bone_vec) as f64 > AIM_DISTANCE {
                    continue;
                }
            }

            // No bullet velocity/target velocity in this pipeline; keep raw target as-is.
            let raw_predicted = bone_location;

            let target_location = match world_to_screen(&view_matrix, raw_predicted) {
                Some(screen) => screen,
                None => continue,
            };

            if !target_location.0.is_finite() || !target_location.1.is_finite() {
                continue;
            }

            let angle_dist = ((SCREEN_CENTER_X - target_location.0).powi(2)
                + (SCREEN_CENTER_Y - target_location.1).powi(2))
            .sqrt();

            if !angle_dist.is_finite() || angle_dist > AIM_FOV {
                continue;
            }

            if best_target.is_none() || angle_dist < best_angle_dist.unwrap_or(f64::MAX) {
                best_target = Some(target_location);
                best_angle_dist = Some(angle_dist);
                best_target_id = idx + 1;
                best_raw_predicted = Some(raw_predicted);
            }
        }
    }

    let best_target = if let (Some(target), Some(raw_pred)) = (best_target, best_raw_predicted) {
        if HUMANIZE_AIM && PREDICTION_SMOOTHING > 0.0 {
            let smoothed_pred = AIM_STATE.with(|state| {
                let mut state = state.borrow_mut();

                if let Some(last_pred) = state.last_predicted_pos {
                    let alpha = 1.0 - PREDICTION_SMOOTHING.clamp(0.0, 1.0);
                    let smoothed = FVector {
                        x: last_pred.x + (raw_pred.x - last_pred.x) * alpha,
                        y: last_pred.y + (raw_pred.y - last_pred.y) * alpha,
                        z: last_pred.z + (raw_pred.z - last_pred.z) * alpha,
                    };
                    state.last_predicted_pos = Some(smoothed);
                    smoothed
                } else {
                    state.last_predicted_pos = Some(raw_pred);
                    raw_pred
                }
            });

            world_to_screen(&view_matrix, smoothed_pred)
                .filter(|(x, y)| x.is_finite() && y.is_finite())
                .or(Some(target))
        } else {
            Some(target)
        }
    } else {
        None
    };

    if best_target.is_none() && HUMANIZE_AIM {
        AIM_STATE.with(|state| {
            let mut state = state.borrow_mut();
            if let Some(last_seen) = state.last_target_seen_at
                && last_seen.elapsed() > Duration::from_millis(TARGET_LOST_RESET_MS)
            {
                state.last_target_id = 0;
                state.target_acquired_at = None;
                reset_human_state(&mut state);
            }
        });
    }

    let Some(target) = best_target else {
        return ThreadFlow::Continue;
    };

    if HUMANIZE_AIM {
        let should_delay = AIM_STATE.with(|state| {
            let mut state = state.borrow_mut();
            let now = Instant::now();

            let reaction_low = REACTION_MIN_MS.min(REACTION_MAX_MS).max(0.0);
            let reaction_high = REACTION_MIN_MS.max(REACTION_MAX_MS).max(0.0);

            if best_target_id != state.last_target_id {
                let mut apply_delay = true;
                if let Some(last_seen) = state.last_target_seen_at
                    && now.duration_since(last_seen) <= Duration::from_millis(TARGET_LOST_RESET_MS)
                {
                    apply_delay = false;
                }
                if state.last_target_id == 0 {
                    apply_delay = true;
                }

                if apply_delay {
                    let mut rng = rand::rng();
                    let delay_ms = rng.random_range(reaction_low..=reaction_high);
                    state.reaction_delay = Duration::from_millis(delay_ms as u64);
                    state.target_acquired_at = Some(now);
                } else {
                    state.reaction_delay = Duration::ZERO;
                    state.target_acquired_at = None;
                }

                state.last_target_id = best_target_id;
                state.last_predicted_pos = None;

                let mut rng = rand::rng();
                state.shake_phase_x = rng.random_range(0.0..=6.28);
                state.shake_phase_y = rng.random_range(0.0..=6.28);
                state.shake_time = 0.0;

                state.velocity_x = 0.0;
                state.velocity_y = 0.0;
            }

            state.last_target_seen_at = Some(now);

            if let Some(acquired_time) = state.target_acquired_at
                && acquired_time.elapsed() < state.reaction_delay
            {
                return true;
            }

            false
        });

        if should_delay {
            return ThreadFlow::Continue;
        }
    }

    let x = target.0 - SCREEN_CENTER_X;
    let y = target.1 - SCREEN_CENTER_Y;
    if !x.is_finite() || !y.is_finite() {
        AIM_STATE.with(|state| {
            let mut state = state.borrow_mut();
            reset_human_state(&mut state);
        });
        return ThreadFlow::Continue;
    }

    let distance_to_target = (x * x + y * y).sqrt();

    let mut effective_smoothing = AIM_SMOOTHING;
    if HUMANIZE_AIM && DYNAMIC_SMOOTHING {
        let distance_factor = (distance_to_target / 200.0).clamp(0.5, 1.5);
        effective_smoothing *= distance_factor;
    }

    let (x, y) = if HUMANIZE_AIM {
        AIM_STATE.with(|state| {
            let mut state = state.borrow_mut();

            let error_x = x;
            let error_y = y;

            let spring_strength = effective_smoothing.clamp(0.05, 0.35);
            state.velocity_x += (error_x - state.velocity_x) * spring_strength;
            state.velocity_y += (error_y - state.velocity_y) * spring_strength;

            let damping = 0.88 - (OVERSHOOT_CHANCE.clamp(0.0, 100.0) / 100.0) * 0.23;
            state.velocity_x *= damping;
            state.velocity_y *= damping;

            if !state.velocity_x.is_finite() || !state.velocity_y.is_finite() {
                reset_human_state(&mut state);
                return (0.0, 0.0);
            }

            (state.velocity_x, state.velocity_y)
        })
    } else {
        (x * effective_smoothing, y * effective_smoothing)
    };

    let (move_x, move_y) = if HUMANIZE_AIM {
        AIM_STATE.with(|state| {
            let mut state = state.borrow_mut();

            state.mouse_accumulator_x += x;
            state.mouse_accumulator_y += y;
            if !state.mouse_accumulator_x.is_finite() || !state.mouse_accumulator_y.is_finite() {
                reset_human_state(&mut state);
                return (0, 0);
            }

            let int_x = state.mouse_accumulator_x.trunc();
            let int_y = state.mouse_accumulator_y.trunc();

            state.mouse_accumulator_x -= int_x;
            state.mouse_accumulator_y -= int_y;

            (int_x as i32, int_y as i32)
        })
    } else {
        (x.floor() as i32, y.floor() as i32)
    };

    let (_, pressed) = read_mouse_state();
    if pressed {
        move_mouse(move_x, move_y);
    }
    if soul_target_found {
        //force click
        unsafe {
            SendInput(
                &[INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags:
                                windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }],
                std::mem::size_of::<INPUT>() as i32,
            );
            std::thread::sleep(Duration::from_millis(2));
            SendInput(
                &[INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: 0,
                            dwFlags:
                                windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }],
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }

    ThreadFlow::Continue
}
