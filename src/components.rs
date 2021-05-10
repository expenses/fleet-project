use ultraviolet::{Mat3, Rotor3, Vec3};

pub struct Position(pub Vec3);
pub struct Rotation(pub Rotor3);
#[derive(Default)]
pub struct RotationMatrix {
    pub matrix: Mat3,
    pub reversed: Mat3,
    pub rotated_model_min: Vec3,
    pub rotated_model_max: Vec3,
}

pub struct Selected;

#[derive(Copy, Clone)]
pub enum ModelId {
    Carrier = 0,
    Fighter = 1,
}
