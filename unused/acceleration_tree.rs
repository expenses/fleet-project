use crate::resources::Ray;
use legion::Entity;
use ultraviolet::{Isometry3, Vec3};

pub struct ShipBoundingBox {
    aabb: rstar::AABB<[f32; 3]>,
    entity: Entity,
}

impl ShipBoundingBox {
    pub fn new(min: Vec3, max: Vec3, transform: Isometry3, entity: Entity) -> Self {
        let min = transform * min;
        let max = transform * max;

        Self {
            aabb: rstar::AABB::from_corners(min.into(), max.into()),
            entity,
        }
    }
}

impl rstar::RTreeObject for ShipBoundingBox {
    type Envelope = rstar::AABB<[f32; 3]>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

impl PartialEq for ShipBoundingBox {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl rstar::SelectionFunction<ShipBoundingBox> for &Ray {
    fn should_unpack_parent(&self, envelope: &rstar::AABB<[f32; 3]>) -> bool {
        self.bounding_box_intersection(envelope.lower().into(), envelope.upper().into())
            .is_some()
    }

    fn should_unpack_leaf(&self, bounding_box: &ShipBoundingBox) -> bool {
        self.bounding_box_intersection(
            bounding_box.aabb.lower().into(),
            bounding_box.aabb.upper().into(),
        )
        .is_some()
    }
}

#[derive(Default)]
pub struct AccelerationTree {
    tree: rstar::RTree<ShipBoundingBox>,
}

impl AccelerationTree {
    pub fn replace(&mut self, bounding_boxes: Vec<ShipBoundingBox>) {
        self.tree = rstar::RTree::bulk_load(bounding_boxes);
    }

    pub fn locate<'a>(&'a self, ray: &'a Ray) -> impl Iterator<Item = Entity> + 'a {
        self.tree
            .locate_with_selection_function(ray)
            .map(|bb| bb.entity)
    }
}
