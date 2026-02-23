use newbase::DoubleBuffer;
use unreal_esp::{BoxPosition, EspPlayer, Skeleton, Text, color32_to_u32};
use unreal_types::ue5::FVector;

use crate::models::math::Vector3;
use crate::models::{AbilitySlot, EntityType, Hero};

#[derive(Debug, Clone, Copy)]
pub struct Ability {
    pub slot: AbilitySlot,
    pub cooling_down: bool,
    pub channeling: bool,
    pub cooldown_start: f32,
    pub cooldown_end: f32,
    pub data_ptr: usize,
}

impl Default for Ability {
    fn default() -> Self {
        Self {
            slot: AbilitySlot::ESlot_None,
            cooling_down: false,
            channeling: false,
            cooldown_start: 0.0,
            cooldown_end: 0.0,
            data_ptr: 0,
        }
    }
}

impl Ability {
    pub fn new() -> Self {
        Self::default()
    }
}

fn lerp_color(from: [u8; 3], to: [u8; 3], t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    color32_to_u32(
        lerp(from[0], to[0]),
        lerp(from[1], to[1]),
        lerp(from[2], to[2]),
        255,
    )
}

#[derive(Debug, Default, Clone)]
pub struct Player {
    pub pos: Vector3,
    pub alive: bool,
    pub health: i32,
    pub max_health: i32,
    pub is_local: bool,
    pub team_id: i32,
    pub ult_cd: f32,
    pub hero: Hero,
    pub bones: Vec<FVector>,
    pub skeleton_links: [Vec<usize>; 5],
    pub abilities: Vec<Ability>,
}

impl EspPlayer for Player {
    fn bones(&self) -> &Vec<FVector> {
        &self.bones
    }

    fn skeleton(&self) -> Skeleton {
        Skeleton::new(self.skeleton_links.clone())
    }

    fn text(&self) -> Vec<Text> {
        vec![
            Text {
                position: BoxPosition::Top,
                text: self.hero.to_string(),
                size: 14.0,
                scale_with_distance: true,
            },
            Text {
                position: BoxPosition::Top,
                text: self.team_id.to_string(),
                size: 14.0,
                scale_with_distance: true,
            },
        ]
    }

    fn health(&self) -> f32 {
        self.health as f32
    }

    fn max_health(&self) -> f32 {
        self.max_health as f32
    }

    fn bone_color(&self) -> u32 {
        let max = self.max_health.max(1) as f32;
        let health_ratio = (self.health.max(0) as f32 / max).clamp(0.0, 1.0);
        // Low health -> red, high health -> green
        lerp_color([255, 64, 64], [64, 255, 64], health_ratio)
    }

    fn is_local(&self) -> bool {
        self.is_local
    }
}

#[derive(Debug, Default, Clone)]
pub struct Entity {
    pub pos: Vector3,
    pub name: String,
    pub e_type: EntityType,
    pub visible: bool,
    pub attackable: bool,
}

pub struct AppData {
    pub player_buf: DoubleBuffer<Player>,
    pub entity_buf: DoubleBuffer<Entity>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            player_buf: DoubleBuffer::with_capacity(32),
            entity_buf: DoubleBuffer::with_capacity(300),
        }
    }
}
