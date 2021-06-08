use crate::resources::BoundingBox;
use crate::utils::uniform_sphere_distribution;
use bevy_ecs::prelude::Bundle;
use bevy_ecs::prelude::Entity;
use rand::Rng;
use std::collections::VecDeque;
use ultraviolet::{Mat3, Rotor3, Vec3};

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

pub fn base_ship_components(position: Vec3, crew: Vec<Entity>) -> impl Bundle {
    (
        Position(position),
        OnBoard(crew),
        Rotation(Default::default()),
        RotationMatrix::default(),
        WorldSpaceBoundingBox::default(),
        Velocity(Vec3::zero()),
        StagingPersuitForce(Vec3::zero()),
        StagingEvasionForce(Vec3::zero()),
        StagingAvoidanceForce(Vec3::zero()),
        CommandQueue::default(),
        Selectable,
    )
}

pub fn fighter_components(ray_cooldown: f32) -> impl Bundle {
    (
        ModelId::Fighter,
        CanAttack,
        CanBeCarried,
        MaxSpeed(10.0),
        Health(50.0),
        MaxHealth(50.0),
        RayCooldown(ray_cooldown),
        AgroRange(200.0),
    )
}

pub fn miner_components() -> impl Bundle {
    (
        ModelId::Miner,
        CanBeCarried,
        MaxSpeed(15.0),
        Health(40.0),
        MaxHealth(40.0),
        CanMine,
        StoredMinerals {
            stored: 0.0,
            capacity: 10.0,
        },
    )
}

pub fn carrier_components(queue: BuildQueue) -> impl Bundle {
    (
        ModelId::Carrier,
        Carrying::default(),
        MaxSpeed(5.0),
        Health(125.0),
        MaxHealth(250.0),
        queue,
    )
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
    pub minerals: f32,
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

#[derive(Default)]
pub struct BuildQueue {
    building: VecDeque<ShipType>,
    time_of_next_pop: f32,
}

impl BuildQueue {
    pub fn advance(&mut self, total_time: f32) -> Option<ShipType> {
        if let Some(building) = self.building.front().cloned() {
            if total_time > self.time_of_next_pop {
                self.building.pop_front();

                if let Some(next) = self.building.front().cloned() {
                    self.time_of_next_pop = total_time + next.build_time();
                }

                return Some(building);
            }
        }

        None
    }

    pub fn progress_time(&self, total_time: f32) -> Option<f32> {
        if let Some(building) = self.building.front().cloned() {
            let remaining = self.time_of_next_pop - total_time;
            Some(1.0 - (remaining / building.build_time()))
        } else {
            None
        }
    }

    pub fn push(&mut self, to_build: ShipType, total_time: f32) {
        if self.building.is_empty() {
            self.time_of_next_pop = total_time + to_build.build_time();
        }

        self.building.push_back(to_build);
    }

    pub fn queue_length(&self, total_time: f32) -> f32 {
        let mut sum = self
            .building
            .iter()
            .skip(1)
            .map(|model_id| model_id.build_time())
            .sum();

        if !self.building.is_empty() {
            let remaining = self.time_of_next_pop - total_time;
            sum += remaining;
        }

        sum
    }

    pub fn num_in_queue(&self) -> usize {
        self.building.len()
    }
}

#[test]
fn test_build_queue() {
    let mut build_queue = BuildQueue::default();
    build_queue.push(ModelId::Fighter, 0.0);
    assert_eq!(build_queue.progress_time(0.0), Some(0.0));
    assert_eq!(build_queue.progress_time(2.5), Some(0.5));
    assert_eq!(build_queue.progress_time(5.0), Some(1.0));
    build_queue.push(ModelId::Fighter, 0.0);
    assert_eq!(build_queue.queue_length(2.5), 7.5);
}

pub struct DebugWatch;
