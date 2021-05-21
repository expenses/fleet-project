use crate::resources::BoundingBox;
use legion::Entity;
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

#[derive(Copy, Clone)]
pub enum ModelId {
    Carrier = 0,
    Fighter = 1,
    Explosion = 2,
    Asteroid = 3,
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

pub struct MovingTo(pub Vec3);

pub struct FollowsCommands;

pub struct CameraFollowing;

pub struct Friendly;
pub struct Enemy;

pub struct Targetting(pub Entity);
pub struct Evading(pub Entity);

pub struct Velocity(pub Vec3);
pub struct StagingVelocity(pub Vec3);
pub struct RayCooldown(pub f32);

pub struct ProjectileIgnores(pub Entity);
