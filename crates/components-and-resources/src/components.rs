use crate::resources::BoundingBox;
use bevy_ecs::prelude::Entity;
use std::collections::VecDeque;
use ultraviolet::{Mat3, Rotor3, Vec3};

pub struct Position(pub Vec3);
pub struct Rotation(pub Rotor3);

#[derive(Default)]
pub struct RotationMatrix {
    pub matrix: Mat3,
    pub reversed: Mat3,
    pub rotated_model_bounding_box: BoundingBox,
}

pub struct Selected;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ModelId {
    Carrier = 0,
    Fighter = 1,
    Miner = 2,
    Explosion = 3,
    Asteroid = 4,
}

pub struct Scale(pub f32);
pub struct Expands;

pub struct AliveUntil(pub f32);

#[derive(Default)]
pub struct WorldSpaceBoundingBox(pub BoundingBox);

pub struct MaxSpeed(pub f32);

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

pub struct FollowsCommands;

pub struct CameraFollowing;

#[derive(Default)]
pub struct Friendly;
#[derive(Default)]
pub struct Enemy;

pub struct Evading(pub Entity);

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
