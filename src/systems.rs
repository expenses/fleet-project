use crate::components::*;
use crate::gpu_structs::{BackgroundVertex, Instance};
use crate::resources::*;
use ultraviolet::Vec3;

use legion::query::*;
use legion::*;

#[system(for_each)]
#[filter(maybe_changed::<Rotation>())]
pub fn update_ship_rotation_matrix(
    rotation: &Rotation,
    rotation_matrix: &mut RotationMatrix,
    model_id: &ModelId,
    #[resource] models: &Models,
) {
    let matrix = rotation.0.into_matrix();

    let model = models.get(*model_id);

    *rotation_matrix = RotationMatrix {
        matrix,
        reversed: rotation.0.reversed().into_matrix(),
        rotated_model_bounding_box: model.bounding_box.rotate(matrix),
    };
}

#[system(for_each)]
#[filter(component::<Moving>())]
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
    #[resource] gpu_interface: &GpuInterface,
) {
    buffer.upload(&gpu_interface.device, &gpu_interface.queue);
}

#[system]
pub fn clear_ship_buffer(#[resource] buffer: &mut ShipBuffer) {
    buffer.clear();
}

#[system]
pub fn upload_ship_buffer(
    #[resource] buffer: &mut ShipBuffer,
    #[resource] gpu_interface: &GpuInterface,
) {
    buffer.upload(&gpu_interface.device, &gpu_interface.queue);
}

#[system(for_each)]
#[filter(!component::<Scale>())]
pub fn upload_ship_instances(
    entity: &Entity,
    selected: Option<&Selected>,
    position: &Position,
    rotation_matrix: &RotationMatrix,
    model_id: &ModelId,
    #[resource] ship_under_cursor: &ShipUnderCursor,
    #[resource] ship_buffer: &mut ShipBuffer,
) {
    let colour = if ship_under_cursor.0 == Some(*entity) {
        Vec3::one()
    } else if selected.is_some() {
        Vec3::unit_y()
    } else {
        Vec3::zero()
    };

    ship_buffer.stage(
        Instance {
            translation: position.0,
            rotation: rotation_matrix.matrix,
            colour,
            scale: 1.0,
        },
        *model_id as usize,
    );
}

#[system(for_each)]
pub fn upload_scaled_instances(
    position: &Position,
    rotation_matrix: &RotationMatrix,
    model_id: &ModelId,
    scale: &Scale,
    #[resource] ship_buffer: &mut ShipBuffer,
) {
    ship_buffer.stage(
        Instance {
            translation: position.0,
            rotation: rotation_matrix.matrix,
            colour: Vec3::zero(),
            scale: scale.0,
        },
        *model_id as usize,
    );
}

#[system]
pub fn find_ship_under_cursor(
    world: &legion::world::SubWorld,
    query: &mut Query<(
        Entity,
        &WorldSpaceBoundingBox,
        &ModelId,
        &Position,
        &RotationMatrix,
    )>,
    #[resource] ray: &Ray,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &mut ShipUnderCursor,
) {
    ship_under_cursor.0 = query
        .iter(world)
        .filter(|(_, bounding_box, ..)| ray.bounding_box_intersection(bounding_box.0).is_some())
        .flat_map(|(entity, _, model_id, position, rotation)| {
            let ray = ray.centered_around_transform(position.0, rotation.reversed);

            models
                .get(*model_id)
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                .map(move |(_, t)| (entity, t))
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| *entity);
}

#[system]
pub fn debug_find_ship_under_cursor(
    world: &legion::world::SubWorld,
    query: &mut Query<(&ModelId, &Position, &RotationMatrix)>,
    #[resource] ray: &Ray,
    #[resource] models: &Models,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
) {
    if let Some((tri, t, position, rotation)) = query
        .iter(world)
        .filter(|(_, position, rotation)| {
            ray.bounding_box_intersection(rotation.rotated_model_bounding_box + position.0)
                .is_some()
        })
        .flat_map(|(model_id, position, rotation)| {
            let ray = ray.centered_around_transform(position.0, rotation.reversed);

            models
                .get(*model_id)
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                .map(move |(tri, t)| (tri, t, position, rotation))
        })
        .min_by(|(_, a, ..), (_, b, ..)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    {
        lines_buffer.stage(&[
            BackgroundVertex {
                position: position.0 + rotation.matrix * tri.a,
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_b_a),
                colour: Vec3::unit_y(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_b_a),
                colour: Vec3::unit_y(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_c_a),
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_c_a),
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * tri.a,
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: ray.get_intersection_point(t) - Vec3::broadcast(0.5),
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: ray.get_intersection_point(t) + Vec3::broadcast(0.5),
                colour: Vec3::unit_x(),
            },
        ]);
    }
}

#[system]
pub fn update_ray(
    #[resource] dimensions: &Dimensions,
    #[resource] orbit: &Orbit,
    #[resource] perspective_view: &PerspectiveView,
    #[resource] mouse_state: &MouseState,
    #[resource] ray: &mut Ray,
    #[resource] camera: &Camera,
) {
    *ray = Ray::new_from_screen(
        mouse_state.position,
        dimensions.width,
        dimensions.height,
        orbit.as_vector() + camera.center,
        perspective_view,
    );
}

type HasComponent<T> = EntityFilterTuple<ComponentFilter<T>, Passthrough>;

#[system]
pub fn handle_clicks(
    world: &legion::world::SubWorld,
    command_buffer: &mut legion::systems::CommandBuffer,
    selected: &mut Query<Entity, HasComponent<Selected>>,
    #[resource] mouse_button: &MouseState,
    #[resource] ship_under_cursor: &ShipUnderCursor,
) {
    if mouse_button.left_state.was_clicked() {
        selected.for_each(world, |entity| {
            command_buffer.remove_component::<Selected>(*entity);
        });

        if let Some(entity) = ship_under_cursor.0 {
            command_buffer.add_component(entity, Selected);
        }
    }
}

#[system]
pub fn update_mouse_state(
    #[resource] mouse_state: &mut MouseState,
    #[resource] delta_time: &DeltaTime,
) {
    mouse_state.left_state.update(delta_time.0, 0.1);
    mouse_state.right_state.update(delta_time.0, 0.0);
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

#[system]
pub fn move_camera(
    #[resource] keyboard_state: &KeyboardState,
    #[resource] orbit: &Orbit,
    #[resource] perspective_view: &mut PerspectiveView,
    #[resource] camera: &mut Camera,
) {
    keyboard_state.move_camera(camera, orbit);
    perspective_view.set_view(orbit.as_vector(), camera.center);
}

#[system]
pub fn update_keyboard_state(#[resource] keyboard_state: &mut KeyboardState) {
    keyboard_state.update();
}

#[system]
pub fn set_camera_following(
    #[resource] keyboard_state: &KeyboardState,
    #[resource] camera: &mut Camera,
    selected: &mut Query<Entity, HasComponent<Selected>>,
    world: &legion::world::SubWorld,
) {
    if keyboard_state.center_camera.0 {
        camera.following = selected
            .iter(world)
            .next()
            .cloned()
            // If we deselect everything and press 'center camera while following
            // something, it makes the most sense to keep following that thing.
            .or(camera.following);
    }
}

#[system]
pub fn move_camera_around_following(
    #[resource] camera: &mut Camera,
    positions: &mut Query<&Position>,
    world: &legion::world::SubWorld,
) {
    if let Some(following) = camera.following {
        match positions.get(world, following) {
            Ok(position) => camera.center = position.0,
            Err(_) => camera.following = None,
        }
    }
}

#[system]
pub fn spawn_projectiles(
    #[resource] ray: &Ray,
    #[resource] keyboard_state: &KeyboardState,
    #[resource] total_time: &TotalTime,
    command_buffer: &mut legion::systems::CommandBuffer,
) {
    if keyboard_state.fire {
        command_buffer.push((Projectile::new(ray, 10.0), AliveUntil(total_time.0 + 30.0)));
    }
}

#[system(for_each)]
pub fn render_projectiles(
    projectile: &Projectile,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
) {
    let (start, end) = projectile.line_points(5.0);

    lines_buffer.stage(&[
        BackgroundVertex {
            position: start,
            colour: Vec3::unit_x(),
        },
        BackgroundVertex {
            position: end,
            colour: Vec3::unit_y(),
        },
    ]);
}

#[system(for_each)]
pub fn update_projectiles(projectile: &mut Projectile, #[resource] delta_time: &DeltaTime) {
    projectile.update(delta_time.0);
}

#[system]
pub fn collide_projectiles(
    projectiles: &mut Query<(Entity, &Projectile)>,
    ships: &mut Query<(&WorldSpaceBoundingBox, &Position, &RotationMatrix, &ModelId)>,
    world: &legion::world::SubWorld,
    #[resource] models: &Models,
    #[resource] delta_time: &DeltaTime,
    #[resource] total_time: &TotalTime,
    command_buffer: &mut legion::systems::CommandBuffer,
) {
    projectiles.for_each(world, |(entity, projectile)| {
        let bounding_box = projectile.bounding_box(delta_time.0);

        let first_hit = ships
            .iter(world)
            .filter(|(ship_bounding_box, ..)| bounding_box.intersects(ship_bounding_box.0))
            .flat_map(|(_, position, rotation, model_id)| {
                let ray = projectile
                    .as_limited_ray(delta_time.0)
                    .centered_around_transform(position.0, rotation.reversed);

                models
                    .get(*model_id)
                    .acceleration_tree
                    .locate_with_selection_function_with_data(ray)
                    .map(move |(_, t)| t)
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some(t) = first_hit {
            let position = projectile.get_intersection_point(t);

            command_buffer.remove(*entity);
            command_buffer.push((
                Position(position),
                RotationMatrix::default(),
                ModelId::Explosion,
                Scale(0.0),
                AliveUntil(total_time.0 + 5.0),
            ));
        }
    });
}

#[system(for_each)]
pub fn scale_explosions(scale: &mut Scale, #[resource] delta_time: &DeltaTime) {
    scale.0 += delta_time.0;
}

#[system(for_each)]
pub fn kill_temporary(
    entity: &Entity,
    alive_until: &AliveUntil,
    #[resource] total_time: &TotalTime,
    command_buffer: &mut legion::systems::CommandBuffer,
) {
    if total_time.0 > alive_until.0 {
        command_buffer.remove(*entity);
    }
}

#[system]
pub fn increase_total_time(
    #[resource] total_time: &mut TotalTime,
    #[resource] delta_time: &DeltaTime,
) {
    total_time.0 += delta_time.0;
}

// We cache these because it's 6 f32 adds and that adds time to bounding box checks
// if we do them per ray.
#[system(for_each)]
pub fn set_world_space_bounding_box(
    bounding_box: &mut WorldSpaceBoundingBox,
    position: &Position,
    rotation: &RotationMatrix,
) {
    bounding_box.0 = rotation.rotated_model_bounding_box + position.0;
}
