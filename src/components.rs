use crate::resources::BoundingBox;
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
}

pub struct Scale(pub f32);

pub struct Moving;

pub struct AliveUntil(pub f32);

#[derive(Default)]
pub struct WorldSpaceBoundingBox(pub BoundingBox);

pub struct MaxSpeed(pub f32);
