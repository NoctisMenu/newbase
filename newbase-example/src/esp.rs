use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use newbase::{App, LogicSystem, ThreadCtx, ThreadFlow, logic_system, read, skip_err};
use unreal_types::ue5::{FMinimalViewInfo, FVector, FRotator, Vector2};

use crate::models::Hero;
use crate::models::math::{Matrix, Vector3};
use crate::offsets::{client_base, resolved_offsets};
use crate::player::{AppData, Player};

static SKELETON_CACHE: OnceLock<Mutex<HashMap<usize, [Vec<usize>; 5]>>> = OnceLock::new();

fn read_c_string(addr: usize, max_len: usize) -> Option<String> {
    if addr == 0 {
        return None;
    }

    let mut buf = Vec::with_capacity(max_len);
    let mut terminated = false;
    for offset in 0..max_len {
        let byte = read::<u8>(addr + offset).ok()?;
        if byte == 0 {
            terminated = true;
            break;
        }
        buf.push(byte);
    }

    if !terminated || buf.is_empty() {
        return None;
    }

    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn build_skeleton_links(bone_map: &HashMap<String, usize>) -> [Vec<usize>; 5] {
    let mut spine = Vec::with_capacity(5);
    for name in ["pelvis", "spine_0", "spine_1", "neck_0", "head"] {
        if let Some(&idx) = bone_map.get(name) {
            spine.push(idx);
        }
    }

    let arm_root = bone_map
        .get("neck_0")
        .copied()
        .or_else(|| bone_map.get("spine_1").copied())
        .or_else(|| bone_map.get("spine_0").copied());

    let mut left_arm = Vec::with_capacity(5);
    if let Some(root) = arm_root {
        left_arm.push(root);
    }
    for name in ["clavicle_L", "arm_upper_L", "arm_lower_L", "hand_L"] {
        if let Some(&idx) = bone_map.get(name) {
            left_arm.push(idx);
        }
    }

    let mut right_arm = Vec::with_capacity(5);
    if let Some(root) = arm_root {
        right_arm.push(root);
    }
    for name in ["clavicle_R", "arm_upper_R", "arm_lower_R", "hand_R"] {
        if let Some(&idx) = bone_map.get(name) {
            right_arm.push(idx);
        }
    }

    let mut left_leg = Vec::with_capacity(4);
    for name in ["pelvis", "leg_upper_L", "leg_lower_L", "ankle_L"] {
        if let Some(&idx) = bone_map.get(name) {
            left_leg.push(idx);
        }
    }

    let mut right_leg = Vec::with_capacity(4);
    for name in ["pelvis", "leg_upper_R", "leg_lower_R", "ankle_R"] {
        if let Some(&idx) = bone_map.get(name) {
            right_leg.push(idx);
        }
    }

    [spine, left_arm, right_arm, left_leg, right_leg]
}

#[logic_system(name = "esp")]
fn esp(
    app: &mut App<AppData>,
    ui: &newoverlay::imgui::Ui,
    draw_list: &newoverlay::imgui::DrawListMut,
) {
    let offsets = resolved_offsets();
    let client_base = client_base();
    let dw_viewmatrix = read::<Matrix>(client_base + offsets[1]).unwrap();
    let matrix = Matrix::transpose(dw_viewmatrix);
    let viewport = Matrix::get_viewport(
        (0, 0),
        (
            app.window_info.size.0 as i32,
            app.window_info.size.1 as i32,
        ),
    );
    let viewmatrix = matrix * viewport;

    let players = app.state.player_buf.read();
    if players.is_empty() {
        return;
    }

    let window_size = [
        app.window_info.size.0.max(0) as u32,
        app.window_info.size.1.max(0) as u32,
    ];

    let view_info = FMinimalViewInfo {
        location: players
            .first()
            .and_then(|p| p.bones.first().copied())
            .unwrap_or_default(),
        rotation: FRotator::default(),
        fov: 90.0,
    };

    unreal_esp::esp(
        view_info,
        window_size,
        players,
        draw_list,
        ui,
        false,
        |player| player.alive && player.health > 0 && !player.is_local,
        move |position, _view_info, _window_size| {
            let world = Vector3 {
                x: position.x as f32,
                y: position.y as f32,
                z: position.z as f32,
            };
            viewmatrix
                .transform(&world)
                .map(|(x, y)| Vector2 { x, y })
        },
    );
}

pub fn system() -> impl LogicSystem<AppData> {
    Esp
}

pub fn cache_players(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    let state = ctx.state();
    let offsets = resolved_offsets();
    let skeleton_cache = SKELETON_CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let client_base = client_base();
    let entity_list = read::<usize>(client_base + offsets[0]).unwrap();
    let mut players = Vec::with_capacity(20);

    for i in 0..20 {
        let mut player = Player::default();
        let ent_entry = skip_err!(read::<usize>(entity_list + 8 * ((i & 0x7FFF) >> 9) + 16));
        if ent_entry == 0 {
            continue;
        }

        let controller = skip_err!(read::<usize>(ent_entry + 0x70 * (i & 0x1ff)));
        let pawn_handle = skip_err!(read::<usize>(controller + 0x8ac));
        let entry = skip_err!(read::<usize>(
            entity_list + 0x8 * ((pawn_handle & 0x7FFF) >> 9) + 16
        ));
        let pawn = skip_err!(read::<usize>(entry + 0x70 * (pawn_handle & 0x1ff)));

        let scene_node = skip_err!(read::<usize>(pawn + 0x330));
        player.pos = skip_err!(read::<Vector3>(scene_node + 0xc8));
        player.is_local = skip_err!(read::<bool>(controller + 0x778));

        let struct_offset = controller + 0x8f0;
        let hero_id: i32 = skip_err!(read(struct_offset + 0x1c));
        player.alive = skip_err!(read(struct_offset + 0x68));
        player.ult_cd = skip_err!(read(struct_offset + 0x78));
        player.health = skip_err!(read(struct_offset + 0x4c));
        player.max_health = skip_err!(read(struct_offset + 0x10));

        let model_ptr = skip_err!(read::<usize>(scene_node + 0x150 + 0xA0));
        let model = skip_err!(read::<usize>(model_ptr));
        player.skeleton_links = if let Some(cached_links) = skeleton_cache
            .lock()
            .ok()
            .and_then(|cache| cache.get(&model).cloned())
        {
            cached_links
        } else {
            let names_count = skip_err!(read::<i32>(model + 0x178)).max(0) as usize;
            let names = skip_err!(read::<usize>(model + 0x168));
            let mut bone_map: HashMap<String, usize> = HashMap::with_capacity(24);
            for bone_idx in 0..names_count {
                let name_ptr =
                    skip_err!(read::<usize>(names + bone_idx * std::mem::size_of::<usize>()));
                let Some(name) = read_c_string(name_ptr, 96) else {
                    continue;
                };
                match name.as_str() {
                    "head" | "neck_0" | "spine_0" | "spine_1" | "clavicle_L" | "arm_upper_L"
                    | "arm_lower_L" | "hand_L" | "pelvis" | "clavicle_R" | "arm_upper_R"
                    | "arm_lower_R" | "hand_R" | "leg_upper_L" | "leg_lower_L" | "ankle_L"
                    | "leg_upper_R" | "leg_lower_R" | "ankle_R" => {
                        bone_map.entry(name).or_insert(bone_idx);
                        if bone_map.len() >= 19 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let links = build_skeleton_links(&bone_map);
            if let Ok(mut cache) = skeleton_cache.lock() {
                cache.insert(model, links.clone());
            }
            links
        };

        let bone_array = skip_err!(read::<usize>(scene_node + 0x150 + 0x80));
        let max_bone_index = player
            .skeleton_links
            .iter()
            .flat_map(|limb| limb.iter().copied())
            .max()
            .map(|idx| idx + 1)
            .unwrap_or(0)
            .min(540);

        player.bones.reserve(max_bone_index);
        for bone in 0..max_bone_index {
            let Ok(bone_pos) = read::<Vector3>(bone_array + bone * 32) else {
                break;
            };
            player.bones.push(FVector {
                x: bone_pos.x as f64,
                y: bone_pos.y as f64,
                z: bone_pos.z as f64,
            });
        }

        player.hero = match Hero::try_from(hero_id) {
            Ok(hero) => hero,
            Err(_) => {
                log::warn!("Unknown hero index: {}", hero_id);
                Hero::None
            }
        };

        players.push(player);
    }

    state.player_buf.write_from_vec(players);
    ThreadFlow::Continue
}
