use crate::gpu_structs::Instance;
use ultraviolet::Isometry3;

#[derive(Default)]
pub struct ShipTransform(pub Isometry3);

impl ShipTransform {
    pub fn as_instance(&self) -> Instance {
        Instance {
            rotation: self.0.rotation.into_matrix(),
            translation: self.0.translation,
        }
    }
}

pub struct Selected;
