use crate::components::*;
use crate::gpu_structs::{BackgroundVertex, Instance};
use crate::resources::*;
use ultraviolet::Vec3;

use legion::*;

#[system(for_each)]
#[filter(maybe_changed::<ShipTransform>())]
pub fn update_ship_instances(transform: &ShipTransform, instance: &mut Instance) {
    *instance = transform.as_instance();
    println!("Updated");
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
    instance: &Instance,
    #[resource] ship_instance_buffer: &mut GpuBuffer<Instance>,
) {
    ship_instance_buffer.stage(&[*instance]);
}

#[system(for_each)]
pub fn update_ship_bounding_boxes(
    entity: &Entity,
    transform: &ShipTransform,
    selected: Option<&Selected>,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &ShipUnderCursor,
) {
    let colour = if ship_under_cursor.0 == Some(*entity) {
        Some(Vec3::one())
    } else if selected.is_some() {
        Some(Vec3::unit_y())
    } else {
        None
    };

    if let Some(colour) = colour {
        let lines =
            crate::bounding_box_lines(models.carrier.bounding_box_line_points, colour, transform.0);
        lines_buffer.stage(&lines);
    }

    // For Debugging the ray transform
    /*
    lines_buffer.stage(&[
        BackgroundVertex {
            colour: Vec3::one(),
            position: ray.origin,
        },
        BackgroundVertex {
            colour: Vec3::zero(),
            position: ray.origin + ray.direction * 10.0,
        }
    ])
    */
}

#[system]
pub fn find_ship_under_cursor(
    world: &legion::world::SubWorld,
    query: &mut Query<(Entity, &ShipTransform)>,
    #[resource] ray: &Ray,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &mut ShipUnderCursor,
) {
    let mut ray_triangle_intersections = 0;

    ship_under_cursor.0 = query
        .iter(world)
        .flat_map(|(entity, transform)| {
            let ray = ray.centered_around_transform(transform.0);

            models
                .carrier
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                .map(move |(_, t)| (entity, t))
        })
        .inspect(|_| {
            ray_triangle_intersections += 1;
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| *entity);

    //dbg!(ray_triangle_intersections);
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
