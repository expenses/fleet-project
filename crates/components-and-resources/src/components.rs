use crate::resources::BoundingBox;
use crate::utils::uniform_sphere_distribution;
use bevy_ecs::prelude::Bundle;
use bevy_ecs::prelude::Entity;
use rand::Rng;
use std::collections::VecDeque;
use std::f32::consts::TAU;
use ultraviolet::{Mat3, Rotor3, Vec3};

mod build_queue;
mod functions;
mod people;

pub use build_queue::*;
pub use functions::*;
pub use people::*;

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
            rng.gen_range(0.0..TAU),
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

#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn build_cost(self) -> f32 {
        self.build_time() * 5.0
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

#[derive(Clone, Copy, Default)]
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

#[derive(Clone, Copy, Default)]
pub struct Velocity(pub Vec3);
pub struct StagingPersuitForce(pub Vec3);
pub struct StagingEvasionForce(pub Vec3);
pub struct StagingAvoidanceForce(pub Vec3);
pub struct RayCooldown(pub f32);

pub struct AgroRange(pub f32);

#[derive(Default)]
pub struct CommandQueue(pub VecDeque<Command>);

#[derive(Clone, Copy)]
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
pub struct Carrying(arrayvec::ArrayVec<(Entity, bool), 100>);

impl Carrying {
    #[must_use]
    pub fn checked_push(&mut self, entity: Entity, priority: bool) -> bool {
        if self.is_full() {
            return false;
        }

        if priority {
            let insert_index = self.0.partition_point(|&(_, priority)| priority);
            self.0.insert(insert_index, (entity, priority));
        } else {
            self.0.push((entity, priority));
        }

        true
    }

    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().map(|&(entity, _)| entity)
    }

    pub fn drain(&mut self) -> impl Iterator<Item = Entity> + '_ {
        self.0.drain(..).map(|(entity, _)| entity)
    }

    pub fn is_full(&self) -> bool {
        self.0.is_full()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub struct CanBeCarried;

pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { max, current: max }
    }
}

pub struct Selectable;

#[derive(Debug)]
pub struct OnBoard(pub Vec<Entity>);

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

pub struct TlasIndex {
    pub index: usize,
    pub padded_bounding_box: BoundingBox,
}

pub struct CarrierFull;

pub struct ResearchMultiplier(pub f32);
