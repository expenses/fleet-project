use crate::resources::BoundingBox;
use crate::utils::uniform_sphere_distribution;
use bevy_ecs::prelude::Bundle;
use bevy_ecs::prelude::Entity;
use rand::Rng;
use std::collections::VecDeque;
use ultraviolet::{Mat3, Rotor3, Vec3};

mod build_queue;
mod functions;

pub use build_queue::*;
pub use functions::*;

#[derive(Debug)]
pub struct Position(pub Vec3);
pub struct Rotation(pub Rotor3);

#[derive(Default, Debug)]
pub struct RotationMatrix {
    pub matrix: Mat3,
    pub reversed: Mat3,
    pub rotated_model_bounding_box: BoundingBox,
}

impl RotationMatrix {
    pub fn random_for_rendering_only(rng: &mut rand::rngs::SmallRng) -> Self {
        let rotor = Rotor3::from_angle_plane(
            rng.gen_range(0.0..std::f32::consts::TAU),
            ultraviolet::Bivec3::from_normalized_axis(uniform_sphere_distribution(rng)),
        );

        Self {
            matrix: rotor.into_matrix(),
            reversed: Default::default(),
            rotated_model_bounding_box: Default::default(),
        }
    }
}

pub struct Selected;

#[derive(Debug, Clone, Copy)]
pub enum ShipType {
    Carrier,
    Fighter,
    Miner,
}

impl ShipType {
    pub fn build_time(self) -> f32 {
        match self {
            Self::Carrier => 30.0,
            Self::Fighter => 5.0,
            Self::Miner => 7.5,
        }
    }

    pub fn model_id(self) -> ModelId {
        match self {
            Self::Carrier => ModelId::Carrier,
            Self::Fighter => ModelId::Fighter,
            Self::Miner => ModelId::Miner,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModelId {
    Carrier = 0,
    Fighter = 1,
    Miner = 2,
    Explosion = 3,
    Asteroid = 4,
}

pub struct Scale(pub f32);

impl Scale {
    pub fn range_sq(&self) -> f32 {
        let range = self.0 + 10.0;
        range * range
    }
}

pub struct Expands;

pub struct AliveUntil(pub f32);

#[derive(Default)]
pub struct WorldSpaceBoundingBox(pub BoundingBox);

#[derive(Clone, Default)]
pub struct MaxSpeed(pub f32);

impl MaxSpeed {
    pub fn max_force(&self) -> f32 {
        self.0 / 10.0
    }
}

pub struct Spin {
    angle: f32,
    plane: ultraviolet::Bivec3,
}

impl Spin {
    pub fn new(axis: Vec3) -> Self {
        Self {
            angle: 0.0,
            plane: ultraviolet::Bivec3::from_normalized_axis(axis),
        }
    }

    pub fn update_angle(&mut self, amount: f32) {
        self.angle += amount;
    }

    pub fn as_rotor(&self) -> Rotor3 {
        Rotor3::from_angle_plane(self.angle, self.plane)
    }
}

pub struct CameraFollowing;

#[derive(Default)]
pub struct Friendly;
#[derive(Default)]
pub struct Enemy;

pub struct Evading(pub Entity);

#[derive(Clone, Default)]
pub struct Velocity(pub Vec3);
pub struct StagingPersuitForce(pub Vec3);
pub struct StagingEvasionForce(pub Vec3);
pub struct StagingAvoidanceForce(pub Vec3);
pub struct RayCooldown(pub f32);

pub struct AgroRange(pub f32);

#[derive(Default)]
pub struct CommandQueue(pub VecDeque<Command>);

#[derive(Clone)]
pub enum Command {
    MoveTo {
        point: Vec3,
        ty: MoveType,
    },
    Interact {
        target: Entity,
        ty: InteractionType,
        range_sq: f32,
    },
}

#[derive(Copy, Clone)]
pub enum MoveType {
    Normal,
    Attack,
}

#[derive(Copy, Clone)]
pub enum InteractionType {
    BeCarriedBy,
    Attack,
    Mine,
}

pub struct CanAttack;
#[derive(Default)]
pub struct Carrying(pub Vec<Entity>);
pub struct CanBeCarried;
pub struct Health(pub f32);
pub struct MaxHealth(pub f32);

pub struct Selectable;

#[derive(Debug)]
pub struct OnBoard(pub Vec<Entity>);

pub enum PersonType {
    Civilian,
    Engineer,
}

pub struct CanMine;
pub struct CanBeMined {
    pub total: f32,
    pub minerals: f32,
}

impl CanBeMined {
    pub fn new(minerals: f32) -> Self {
        Self {
            total: minerals,
            minerals,
        }
    }
}

pub struct StoredMinerals {
    pub stored: f32,
    pub capacity: f32,
}

pub struct Unloading {
    pub until: f32,
}

impl Unloading {
    pub fn new(total_time: f32) -> Self {
        Self {
            until: total_time + 0.5,
        }
    }
}

pub struct DebugWatch;
