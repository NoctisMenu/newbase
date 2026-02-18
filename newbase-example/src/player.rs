use newbase::DoubleBuffer;
use unreal_esp::{EspPlayer, Skeleton};
use unreal_types::ue5::FVector;

use crate::models::Hero;
use crate::models::math::Vector3;

#[derive(Debug, Default, Clone)]
pub struct Player {
    pub pos: Vector3,
    pub alive: bool,
    pub health: i32,
    pub max_health: i32,
    pub is_local: bool,
    pub ult_cd: f32,
    pub hero: Hero,
    pub bones: Vec<FVector>,
    pub skeleton_links: [Vec<usize>; 5],
}

impl EspPlayer for Player {
    fn bones(&self) -> &Vec<FVector> {
        &self.bones
    }

    fn skeleton(&self) -> Skeleton {
        Skeleton::new(self.skeleton_links.clone())
    }

    fn health(&self) -> f32 {
        self.health as f32
    }

    fn max_health(&self) -> f32 {
        self.max_health as f32
    }
}

pub struct AppData {
    pub player_buf: DoubleBuffer<Player>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            player_buf: DoubleBuffer::with_capacity(32),
        }
    }
}
