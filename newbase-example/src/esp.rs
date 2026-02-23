use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::Duration;

use newbase::{App, LogicSystem, ThreadCtx, ThreadFlow, logic_system, read, skip_err, skip_opt};
use unreal_types::ue5::{FMinimalViewInfo, FRotator, FVector, Vector2};

use crate::models::math::{Matrix, Vector3};
use crate::models::{AbilitySlot, EntityType, Hero};
use crate::offsets::{client_base, resolved_offsets};
use crate::player::{Ability, AppData, Entity, Player};

use crate::offsets::client_dll::cs2_dumper::schemas::client_dll::*;

static SKELETON_CACHE: OnceLock<Mutex<HashMap<usize, [Vec<usize>; 5]>>> = OnceLock::new();
static ENTITY_REF_CACHE: OnceLock<RwLock<Vec<CachedEntityRef>>> = OnceLock::new();
static ENTITY_REF_WORKER: OnceLock<()> = OnceLock::new();
const ENTITY_INDEX_MASK: usize = 0x7FFF;
const ENTITY_PAGE_SIZE: usize = 512;
const ENTITY_ENTRY_STRIDE: usize = 0x70;
const ENTITY_LIST_PAGE_OFFSET: usize = 16;
const ENTITY_SCAN_INTERVAL: Duration = Duration::from_millis(2);

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

fn read_entity_designer_name(entity_identity: usize) -> Option<String> {
    // Some builds expose m_designerName as a pointer at +0x20, while others keep
    // compatible layouts where reading directly still works. Try pointer first.
    read::<usize>(entity_identity + 0x20)
        .ok()
        .and_then(|name_ptr| read_c_string(name_ptr, 96))
        .or_else(|| read_c_string(entity_identity + 0x20, 96))
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

fn draw_world_circle(
    draw_list: &newoverlay::imgui::DrawListMut,
    viewmatrix: &Matrix,
    center: Vector3,
    radius: f32,
    segments: usize,
    color: [f32; 4],
    thickness: f32,
) {
    if radius <= 0.0 || segments < 3 {
        return;
    }

    let step = std::f32::consts::TAU / segments as f32;
    // Build a flat ring on the XY plane in world space, then project each edge.
    for i in 0..segments {
        let angle_a = i as f32 * step;
        let angle_b = (i as f32 + 1.0) * step;
        let world_a = Vector3 {
            x: center.x + angle_a.cos() * radius,
            y: center.y + angle_a.sin() * radius,
            z: center.z,
        };
        let world_b = Vector3 {
            x: center.x + angle_b.cos() * radius,
            y: center.y + angle_b.sin() * radius,
            z: center.z,
        };

        let Some((ax, ay)) = viewmatrix.transform(&world_a) else {
            continue;
        };
        let Some((bx, by)) = viewmatrix.transform(&world_b) else {
            continue;
        };
        if !ax.is_finite() || !ay.is_finite() || !bx.is_finite() || !by.is_finite() {
            continue;
        }

        draw_list
            .add_line([ax, ay], [bx, by], color)
            .thickness(thickness)
            .build();
    }
}

#[derive(Clone)]
struct CachedEntityRef {
    entity_ptr: usize,
    name: String,
    e_type: EntityType,
}

struct TrackedEntityKind {
    name: String,
    e_type: EntityType,
    last_seen_scan: u64,
}

fn classify_entity_name(name: &str) -> Option<EntityType> {
    match name {
        "item_xp" => Some(EntityType::Soul),
        "npc_tro" => Some(EntityType::Creep),
        //todo: troopers
        _ => None,
    }
}

fn read_max_entities(client_base: usize, entity_list: usize) -> Option<usize> {
    if let Ok(game_entity_system) = read::<usize>(
        client_base + crate::offsets::offsets::cs2_dumper::offsets::client_dll::dwGameEntitySystem,
    ) {
        if game_entity_system != 0 {
            if let Ok(highest_index) = read::<i32>(
                game_entity_system
                    + crate::offsets::offsets::cs2_dumper::offsets::client_dll::dwGameEntitySystem_highestEntityIndex,
            ) {
                if highest_index >= 0 {
                    return Some((highest_index as usize + 1).min(ENTITY_INDEX_MASK + 1));
                }
            }
        }
    }

    read::<u32>(entity_list + 0x1534)
        .ok()
        .map(|v| (v as usize).min(ENTITY_INDEX_MASK + 1))
}

fn collect_cached_entities(
    entity_list: usize,
    max_entities: usize,
    tracked_entities: &mut HashMap<usize, TrackedEntityKind>,
    scan_id: u64,
) -> Vec<CachedEntityRef> {
    if max_entities == 0 {
        tracked_entities.clear();
        return Vec::new();
    }

    let mut cached_entities = Vec::with_capacity(500);
    let page_count = max_entities.div_ceil(ENTITY_PAGE_SIZE);

    for page in 0..page_count {
        let ent_entry = skip_err!(read::<usize>(
            entity_list + ENTITY_LIST_PAGE_OFFSET + page * std::mem::size_of::<usize>()
        ));
        if ent_entry == 0 {
            continue;
        }

        let page_base = page * ENTITY_PAGE_SIZE;
        let slots_in_page = (max_entities - page_base).min(ENTITY_PAGE_SIZE);

        for slot in 0..slots_in_page {
            let entity_ptr = skip_err!(read::<usize>(ent_entry + ENTITY_ENTRY_STRIDE * slot));
            if entity_ptr == 0 {
                continue;
            }

            if let Some(tracked) = tracked_entities.get_mut(&entity_ptr) {
                tracked.last_seen_scan = scan_id;
                cached_entities.push(CachedEntityRef {
                    entity_ptr,
                    name: tracked.name.clone(),
                    e_type: tracked.e_type,
                });
                continue;
            }

            let entity_identity = skip_err!(read::<usize>(entity_ptr + 0x10)); //m_pEntity
            let name = skip_opt!(read_entity_designer_name(entity_identity)); //m_designerName
            let Some(e_type) = classify_entity_name(&name) else {
                continue;
            };

            tracked_entities.insert(
                entity_ptr,
                TrackedEntityKind {
                    name: name.clone(),
                    e_type,
                    last_seen_scan: scan_id,
                },
            );

            cached_entities.push(CachedEntityRef {
                entity_ptr,
                name,
                e_type,
            });
        }
    }

    tracked_entities.retain(|_, tracked| tracked.last_seen_scan == scan_id);
    cached_entities
}

fn ensure_entity_ref_worker() -> &'static RwLock<Vec<CachedEntityRef>> {
    let cache = ENTITY_REF_CACHE.get_or_init(|| RwLock::new(Vec::with_capacity(500)));

    ENTITY_REF_WORKER.get_or_init(|| {
        let cache = cache;

        if let Err(error) = std::thread::Builder::new()
            .name("entity-ref-cache".to_string())
            .spawn(move || {
                let offsets = resolved_offsets();
                let client_base = client_base();
                let mut tracked_entities: HashMap<usize, TrackedEntityKind> =
                    HashMap::with_capacity(1024);
                let mut scan_id: u64 = 0;

                loop {
                    scan_id = scan_id.wrapping_add(1);
                    if let Ok(entity_list) = read::<usize>(client_base + offsets[0]) {
                        if let Some(max_entities) = read_max_entities(client_base, entity_list) {
                            let scanned = collect_cached_entities(
                                entity_list,
                                max_entities,
                                &mut tracked_entities,
                                scan_id,
                            );
                            if let Ok(mut guard) = cache.write() {
                                *guard = scanned;
                            }
                        }
                    }

                    std::thread::sleep(ENTITY_SCAN_INTERVAL);
                }
            })
        {
            log::error!("failed to spawn entity-ref-cache thread: {}", error);
        }
    });

    cache
}

#[logic_system(name = "esp")]
fn esp(
    app: &mut App<AppData>,
    ui: &newoverlay::imgui::Ui,
    draw_list: &newoverlay::imgui::DrawListMut,
) {
    const CREEP_RING_COLOR: [f32; 4] = [0.22, 0.92, 0.38, 0.90];
    const CREEP_RING_RADIUS: f32 = 90.0;
    const CREEP_RING_SEGMENTS: usize = 48;
    const CREEP_RING_THICKNESS: f32 = 2.0;
    const SOUL_COLOR: [f32; 4] = [1.0, 0.82, 0.33, 0.95];
    const SOUL_RADIUS_MIN: f32 = 3.0;
    const SOUL_RADIUS_MAX: f32 = 14.0;

    let offsets = resolved_offsets();
    let client_base = client_base();
    let dw_viewmatrix = read::<Matrix>(client_base + offsets[1]).unwrap();
    let matrix = Matrix::transpose(dw_viewmatrix);
    let viewport = Matrix::get_viewport(
        (0, 0),
        (app.window_info.size.0 as i32, app.window_info.size.1 as i32),
    );
    let viewmatrix = matrix * viewport;

    let players = app.state.player_buf.read();
    let entities = app.state.entity_buf.read();
    let camera_pos = players
        .iter()
        .find(|p| p.is_local)
        .map(|p| p.pos)
        .or_else(|| players.first().map(|p| p.pos))
        .unwrap_or_default();
    let local_ability_debug = players
        .iter()
        .find(|p| p.is_local)
        .map(|p| format!("{:#?}", p.abilities))
        .unwrap_or_else(|| "local player not found".to_string());

    for entity in entities.iter() {
        if !entity.visible {
            continue;
        }

        match entity.e_type {
            EntityType::Soul => {
                let Some((screen_x, screen_y)) = viewmatrix.transform(&entity.pos) else {
                    continue;
                };

                let distance = Vector3::distance(camera_pos, entity.pos).max(1.0);
                let radius = (900.0 / distance).clamp(SOUL_RADIUS_MIN, SOUL_RADIUS_MAX);

                draw_list
                    .add_circle([screen_x, screen_y], radius, SOUL_COLOR)
                    .thickness(2.0)
                    .build();

                draw_list.add_text(
                    [screen_x, screen_y + radius + 5.0],
                    SOUL_COLOR,
                    &entity.name,
                );
            }
            EntityType::Creep => {
                draw_world_circle(
                    draw_list,
                    &viewmatrix,
                    Vector3 {
                        x: entity.pos.x,
                        y: entity.pos.y,
                        z: entity.pos.z + 2.0,
                    },
                    CREEP_RING_RADIUS,
                    CREEP_RING_SEGMENTS,
                    CREEP_RING_COLOR,
                    CREEP_RING_THICKNESS,
                );
            }
        }
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

    let local_team_id = players.iter().find(|p| p.is_local).map(|p| p.team_id);

    unreal_esp::esp(
        view_info,
        window_size,
        players,
        draw_list,
        ui,
        false,
        |player| {
            player.alive
                && player.health > 0
                && !player.is_local
                && local_team_id.is_some_and(|team| team != player.team_id)
        },
        move |position, _view_info, _window_size| {
            let world = Vector3 {
                x: position.x as f32,
                y: position.y as f32,
                z: position.z as f32,
            };
            viewmatrix.transform(&world).map(|(x, y)| Vector2 { x, y })
        },
    );
}

pub fn system() -> impl LogicSystem<AppData> {
    Esp
}

pub fn players(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
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

        let scene_node = skip_err!(read::<usize>(pawn + C_BaseEntity::m_pGameSceneNode));
        player.pos = skip_err!(read::<Vector3>(scene_node + CGameSceneNode::m_vecAbsOrigin));
        player.is_local = skip_err!(read::<bool>(
            controller + CBasePlayerController::m_bIsLocalPlayerController
        ));

        player.team_id = skip_err!(read::<i32>(pawn + C_BaseEntity::m_iTeamNum));

        let struct_offset = controller + CCitadelPlayerController::m_PlayerDataGlobal;
        let hero_id: i32 = skip_err!(read(struct_offset + PlayerDataGlobal_t::m_nHeroID));
        player.alive = skip_err!(read(struct_offset + PlayerDataGlobal_t::m_bAlive));
        player.ult_cd = skip_err!(read(
            struct_offset + PlayerDataGlobal_t::m_flUltimateCooldownEnd
        ));
        player.health = skip_err!(read(struct_offset + PlayerDataGlobal_t::m_iHealth));
        player.max_health = skip_err!(read(struct_offset + PlayerDataGlobal_t::m_iHealthMax));

        let model_ptr = skip_err!(read::<usize>(scene_node + 0x150 + 0xA0)); //should be fine
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
                let name_ptr = skip_err!(read::<usize>(
                    names + bone_idx * std::mem::size_of::<usize>()
                ));
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
            .unwrap_or(0);

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

        // let abilities_comp = pawn + C_CitadelPlayerPawn::m_CCitadelAbilityComponent; //m_CCitadelAbilityComponent
        // let abilities = skip_err!(read::<usize>(
        //     abilities_comp + CCitadelAbilityComponent::m_vecAbilities + 0x8
        // )); //m_vecAbilities.ptr

        // player.abilities.reserve(24);
        // for ability_idx in 0..24 {
        //     let handle = skip_err!(read::<u32>(
        //         abilities + ability_idx * std::mem::size_of::<u32>()
        //     ));
        //     if handle == 0 {
        //         continue;
        //     }

        //     let handle = handle as usize;
        //     let ent_entry = skip_err!(read::<usize>(
        //         entity_list + 8 * ((handle & 0x7FFF) >> 9) + 16
        //     ));
        //     if ent_entry == 0 {
        //         continue;
        //     }

        //     let ability_entity = skip_err!(read::<usize>(ent_entry + 0x70 * (handle & 0x1ff)));
        //     if ability_entity == 0 {
        //         continue;
        //     }
        //     let entity_identity = skip_err!(read::<usize>(ability_entity + 0x10)); //m_pEntity
        //     let name = skip_opt!(read_entity_designer_name(entity_identity)); //m_designerName
        //     let mut ability = Ability::new();
        //     ability.slot = skip_err!(read::<AbilitySlot>(
        //         ability_entity + C_CitadelBaseAbility::m_eAbilitySlot
        //     )); //eAbilitySlot
        //     ability.cooling_down = skip_err!(read::<bool>(
        //         ability_entity + C_CitadelBaseAbility::m_bIsCoolingDownInternal
        //     )); //bIsCoolingDownInternal
        //     ability.channeling = skip_err!(read::<bool>(
        //         ability_entity + C_CitadelBaseAbility::m_bChanneling
        //     )); //bChanneling
        //     ability.cooldown_start = skip_err!(read::<f32>(
        //         ability_entity + C_CitadelBaseAbility::m_flCooldownStart
        //     )); //flCooldownStart
        //     ability.cooldown_end = skip_err!(read::<f32>(
        //         ability_entity + C_CitadelBaseAbility::m_flCooldownEnd
        //     )); //flCooldownEnd
        //     ability.data_ptr = skip_err!(read::<usize>(ability_entity + 0x390)); //pSubclassVData
        //     player.abilities.push(ability);
        // }
        players.push(player);
    }

    state.player_buf.write_from_vec(players);
    ThreadFlow::Continue
}

pub fn entities(ctx: &ThreadCtx<AppData>) -> ThreadFlow {
    let state = ctx.state();
    let cache = ensure_entity_ref_worker();
    let cached_entities = cache.read().map(|guard| guard.clone()).unwrap_or_default();
    if cached_entities.is_empty() {
        return ThreadFlow::Continue;
    }
    let mut entities = Vec::with_capacity(cached_entities.len());

    for cached in cached_entities {
        let mut entity = Entity::default();
        let scene_node = skip_err!(read::<usize>(
            cached.entity_ptr + C_BaseEntity::m_pGameSceneNode
        ));
        entity.pos = skip_err!(read::<Vector3>(scene_node + CGameSceneNode::m_vecAbsOrigin));
        entity.visible = !skip_err!(read::<bool>(scene_node + CGameSceneNode::m_bDormant));
        entity.name = cached.name;
        entity.e_type = cached.e_type;
        entity.attackable = match entity.e_type {
            EntityType::Soul => {
                let attackable_time =
                    skip_err!(read::<f32>(cached.entity_ptr + CItemXP::m_flAttackableTime));
                let cur_time =
                    skip_err!(read::<f32>(cached.entity_ptr + C_BaseEntity::m_flSimulationTime));
                attackable_time <= cur_time + 0.2
            }
            EntityType::Creep => false,
        };
        entities.push(entity);
    }

    state.entity_buf.write_from_vec(entities);
    ThreadFlow::Continue
}
