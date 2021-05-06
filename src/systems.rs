use crate::components::*;
use crate::gpu_structs::{BackgroundVertex, Instance};
use crate::resources::*;
use ultraviolet::Vec3;

use legion::*;

#[system(for_each)]
#[filter(maybe_changed::<Rotation>())]
pub fn update_ship_rotation_matrix(rotation: &Rotation, matrix: &mut RotationMatrix) {
    matrix.matrix = rotation.0.into_matrix();
    matrix.reversed = rotation.0.reversed().into_matrix();
}

#[system(for_each)]
pub fn move_ships(position: &mut Position, rotation: &RotationMatrix) {
    position.0 += rotation.matrix * Vec3::new(0.0, 0.0, 0.01);
}

#[system]
pub fn clear_buffer<T: 'static + Copy + bytemuck::Pod>(#[resource] buffer: &mut GpuBuffer<T>) {
    buffer.clear();
}

#[system]
pub fn upload_buffer<T: 'static + Copy + bytemuck::Pod>(
    #[resource] buffer: &mut GpuBuffer<T>,
    #[resource] device: &wgpu::Device,
) {
    buffer.upload(device);
}

#[system(for_each)]
pub fn upload_ship_instances(
    entity: &Entity,
    selected: Option<&Selected>,
    position: &Position,
    rotation_matrix: &RotationMatrix,
    #[resource] ship_under_cursor: &ShipUnderCursor,
    #[resource] ship_instance_buffer: &mut GpuBuffer<Instance>,
) {
    let colour = if ship_under_cursor.0 == Some(*entity) {
        Vec3::one()
    } else if selected.is_some() {
        Vec3::unit_y()
    } else {
        Vec3::zero()
    };

    ship_instance_buffer.stage(&[Instance {
        translation: position.0,
        rotation: rotation_matrix.matrix,
        colour,
    }]);
}

#[system]
pub fn find_ship_under_cursor(
    world: &legion::world::SubWorld,
    query: &mut Query<(Entity, &Position, &RotationMatrix)>,
    #[resource] ray: &Ray,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &mut ShipUnderCursor,
) {
    ship_under_cursor.0 = query
        .iter(world)
        .flat_map(|(entity, position, rotation)| {
            let ray = ray.centered_around_transform(position.0, rotation.reversed);

            models
                .carrier
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                .map(move |(_, t)| (entity, t))
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| *entity);
}

#[system]
pub fn update_ray(
    #[resource] dimensions: &Dimensions,
    #[resource] orbit: &Orbit,
    #[resource] perspective_view: &PerspectiveView,
    #[resource] mouse_position: &MousePosition,
    #[resource] ray: &mut Ray,
) {
    *ray = Ray::new_from_screen(
        mouse_position.0,
        dimensions.width,
        dimensions.height,
        orbit.as_vector(),
        perspective_view,
    );
}

#[system]
pub fn update_ray_plane_point(
    #[resource] ray: &Ray,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
) {
    if let Some(intersection_point) = ray
        .plane_intersection(0.0)
        .map(|t| ray.get_intersection_point(t))
    {
        lines_buffer.stage(&[
            BackgroundVertex {
                position: intersection_point + Vec3::unit_y(),
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: intersection_point,
                colour: Vec3::unit_y(),
            },
            /*
            BackgroundVertex {
                position: ray.origin,
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: ray.origin + ray.direction * 20.0,
                colour: Vec3::unit_y(),
            },
            */
        ]);
    }
}
