use crate::components::*;
use crate::gpu_structs::{BackgroundVertex, CircleInstance, Instance};
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
pub fn upload_instances(
    entity: &Entity,
    selected: Option<&Selected>,
    position: &Position,
    rotation_matrix: &RotationMatrix,
    model_id: &ModelId,
    scale: Option<&Scale>,
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
            scale: get_scale(scale),
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
        Option<&Scale>,
    )>,
    #[resource] ray: &Ray,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &mut ShipUnderCursor,
) {
    ship_under_cursor.0 = query
        .iter(world)
        .filter(|(_, bounding_box, ..)| ray.bounding_box_intersection(bounding_box.0).is_some())
        .flat_map(|(entity, _, model_id, position, rotation, scale)| {
            let scale = get_scale(scale);

            let ray = ray.centered_around_transform(position.0, rotation.reversed, scale);

            models
                .get(*model_id)
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                // We need to multiply t by scale here as the time of impact is calculated on an unscaled model
                .map(move |(_, t)| (entity, t * scale))
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| *entity);
}

#[system]
pub fn debug_find_ship_under_cursor(
    world: &legion::world::SubWorld,
    query: &mut Query<(
        &WorldSpaceBoundingBox,
        &ModelId,
        &Position,
        &RotationMatrix,
        Option<&Scale>,
    )>,
    #[resource] ray: &Ray,
    #[resource] models: &Models,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
) {
    if let Some((tri, _, position, rotation, scale)) = query
        .iter(world)
        .filter(|(bounding_box, ..)| ray.bounding_box_intersection(bounding_box.0).is_some())
        .flat_map(|(_, model_id, position, rotation, scale)| {
            let scale = get_scale(scale);

            let ray = ray.centered_around_transform(position.0, rotation.reversed, scale);

            models
                .get(*model_id)
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                .map(move |(tri, t)| (tri, t * scale, position, rotation, scale))
        })
        .min_by(|(_, a, ..), (_, b, ..)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    {
        lines_buffer.stage(&[
            BackgroundVertex {
                position: position.0 + rotation.matrix * tri.a * scale,
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_b_a) * scale,
                colour: Vec3::unit_y(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_b_a) * scale,
                colour: Vec3::unit_y(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_c_a) * scale,
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_c_a) * scale,
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * tri.a * scale,
                colour: Vec3::unit_x(),
            },
            /*
            BackgroundVertex {
                position: ray.get_intersection_point(t) - Vec3::broadcast(0.5),
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: ray.get_intersection_point(t) + Vec3::broadcast(0.5),
                colour: Vec3::unit_x(),
            },
            */
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
type HasComponents<T> = EntityFilterTuple<And<T>, Passthrough>;
type SelectedFilter = HasComponents<(ComponentFilter<Selected>, ComponentFilter<FollowsCommands>)>;

#[system]
pub fn handle_left_click(
    world: &legion::world::SubWorld,
    command_buffer: &mut legion::systems::CommandBuffer,
    selected: &mut Query<Entity, HasComponent<Selected>>,
    #[resource] mouse_button: &MouseState,
    #[resource] ship_under_cursor: &ShipUnderCursor,
    #[resource] mouse_mode: &mut MouseMode,
    #[resource] keyboard_state: &KeyboardState,
) {
    if mouse_button.left_state.was_clicked() {
        if !keyboard_state.shift {
            selected.for_each(world, |entity| {
                command_buffer.remove_component::<Selected>(*entity);
            });
        }

        *mouse_mode = MouseMode::Normal;

        if let Some(entity) = ship_under_cursor.0 {
            command_buffer.add_component(entity, Selected);
        }
    }
}

#[system]
pub fn handle_right_clicks(
    world: &mut legion::world::SubWorld,
    command_buffer: &mut legion::systems::CommandBuffer,
    selected: &mut Query<(Entity, Option<&mut MovingTo>), SelectedFilter>,
    #[resource] mouse_button: &MouseState,
    #[resource] average_selected_position: &AverageSelectedPosition,
    #[resource] average_selected_end_position: &AverageSelectedEndPosition,
    #[resource] mouse_mode: &mut MouseMode,
    #[resource] ray_plane_point: &RayPlanePoint,
    #[resource] keyboard_state: &KeyboardState,
) {
    if mouse_button.right_state.was_clicked() {
        let avg_position_to_use = if keyboard_state.shift {
            average_selected_end_position.0
        } else {
            average_selected_position.0
        };

        *mouse_mode = match mouse_mode {
            MouseMode::Normal => match avg_position_to_use {
                Some(avg) => MouseMode::Movement { plane_y: avg.y },
                _ => MouseMode::Normal,
            },
            MouseMode::Movement { .. } => {
                if let Some(point) = ray_plane_point.0 {
                    selected.for_each_mut(world, |(entity, moving_to)| {
                        match (moving_to, keyboard_state.shift) {
                            (Some(moving_to), true) => {
                                moving_to.0.push(point);
                            },
                            _ => command_buffer.add_component(*entity, MovingTo(vec![point]))
                        }
                    });
                }

                MouseMode::Normal
            }
        };
    }
}

#[system(for_each)]
pub fn move_ships(
    entity: &Entity,
    position: &mut Position,
    moving_to: &mut MovingTo,
    max_speed: &MaxSpeed,
    command_buffer: &mut legion::systems::CommandBuffer,
    #[resource] delta_time: &DeltaTime,
) {
    if let Some(point) = moving_to.0.first().cloned() {
        let delta = point - position.0;
        let distance = delta.mag();
        let speed = max_speed.0 * delta_time.0;

        if distance < speed {
            position.0 = point;
            moving_to.0.remove(0);
        } else {
            position.0 += delta / distance * speed;
        }
    } else {
        command_buffer.remove_component::<MovingTo>(*entity);
    }
}

#[system(for_each)]
#[filter(maybe_changed::<MovingTo>())]
pub fn set_rotation_from_moving_to(
    position: &Position,
    moving_to: &MovingTo,
    rotation: &mut Rotation,
) {
    if let Some(point) = moving_to.0.first().cloned() {
        let delta = point - position.0;
        let xz_movement = ultraviolet::Vec2::new(delta.x, delta.z).mag();

        rotation.0 = ultraviolet::Rotor3::from_rotation_xz(-delta.x.atan2(delta.z))
            * ultraviolet::Rotor3::from_rotation_yz(-delta.y.atan2(xz_movement));
    }
}

#[system]
pub fn update_mouse_state(
    #[resource] mouse_state: &mut MouseState,
    #[resource] delta_time: &DeltaTime,
) {
    mouse_state.left_state.update(delta_time.0, 0.1);
    mouse_state.right_state.update(delta_time.0, 0.075);
}

#[system]
pub fn update_ray_plane_point(
    #[resource] ray: &Ray,
    #[resource] mouse_mode: &MouseMode,
    #[resource] ray_plane_point: &mut RayPlanePoint,
) {
    ray_plane_point.0 = match *mouse_mode {
        MouseMode::Movement { plane_y } => ray
            .plane_intersection(plane_y)
            .map(|t| ray.get_intersection_point(t)),
        MouseMode::Normal => None,
    };
}

#[system]
pub fn move_camera(
    #[resource] keyboard_state: &KeyboardState,
    #[resource] orbit: &Orbit,
    #[resource] camera: &mut Camera,
) {
    keyboard_state.move_camera(camera, orbit);
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
    #[resource] perspective_view: &mut PerspectiveView,
    #[resource] orbit: &Orbit,
    positions: &mut Query<&Position>,
    world: &legion::world::SubWorld,
) {
    if let Some(following) = camera.following {
        match positions.get(world, following) {
            Ok(position) => camera.center = position.0,
            Err(_) => camera.following = None,
        }
    }

    perspective_view.set_view(orbit.as_vector(), camera.center);
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
    ships: &mut Query<(
        &WorldSpaceBoundingBox,
        &Position,
        &RotationMatrix,
        &ModelId,
        Option<&Scale>,
    )>,
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
            .flat_map(|(_, position, rotation, model_id, scale)| {
                let scale = get_scale(scale);

                let ray = projectile
                    .as_limited_ray(delta_time.0)
                    .centered_around_transform(position.0, rotation.reversed, scale);

                models
                    .get(*model_id)
                    .acceleration_tree
                    .locate_with_selection_function_with_data(ray)
                    .map(move |(_, scaled_t)| scaled_t)
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
                Expands,
            ));
        }
    });
}

#[system(for_each)]
#[filter(component::<Expands>())]
pub fn expand_explosions(scale: &mut Scale, #[resource] delta_time: &DeltaTime) {
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
#[filter(
    maybe_changed::<Position>() | maybe_changed::<RotationMatrix>() | maybe_changed::<Scale>()
)]
pub fn set_world_space_bounding_box(
    bounding_box: &mut WorldSpaceBoundingBox,
    position: &Position,
    rotation: &RotationMatrix,
    scale: Option<&Scale>,
) {
    bounding_box.0 = (rotation.rotated_model_bounding_box * get_scale(scale)) + position.0;
}

#[system(for_each)]
pub fn spin(spin: &mut Spin, rotation: &mut Rotation, #[resource] delta_time: &DeltaTime) {
    spin.update_angle(delta_time.0);
    rotation.0 = spin.as_rotor();
}

fn get_scale(scale: Option<&Scale>) -> f32 {
    scale.map(|scale| scale.0).unwrap_or(1.0)
}

#[system]
pub fn render_movement_circle(
    #[resource] circle_instances: &mut GpuBuffer<CircleInstance>,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
    #[resource] ray_plane_point: &RayPlanePoint,
    #[resource] average_selected_position: &AverageSelectedPosition,
    #[resource] average_selected_end_position: &AverageSelectedEndPosition,
    #[resource] keyboard_state: &KeyboardState,
) {
    let avg_position_to_use = if keyboard_state.shift {
        average_selected_end_position.0
    } else {
        average_selected_position.0
    };

    if let (Some(avg), Some(point)) = (avg_position_to_use, ray_plane_point.0) {
        let mut circle_center = avg;
        circle_center.y = point.y;

        let scale = (point - circle_center).mag();

        let green = Vec3::unit_y();
        let green_alpha = ultraviolet::Vec4::new(0.0, 1.0, 0.0, 0.15);

        circle_instances.stage(&[CircleInstance {
            translation: circle_center,
            scale,
            colour: green_alpha,
        }]);

        lines_buffer.stage(&[
            BackgroundVertex {
                position: avg,
                colour: green,
            },
            BackgroundVertex {
                position: point,
                colour: green,
            },
            BackgroundVertex {
                position: point,
                colour: green,
            },
            BackgroundVertex {
                position: circle_center,
                colour: green,
            },
            BackgroundVertex {
                position: circle_center,
                colour: green,
            },
            BackgroundVertex {
                position: avg,
                colour: green,
            },
        ])
    }
}

#[system]
pub fn calculate_average_selected_position(
    #[resource] average_selected_position: &mut AverageSelectedPosition,
    selected_positions: &mut Query<&Position, SelectedFilter>,
    world: &legion::world::SubWorld,
) {
    let mut count = 0;
    let mut sum = Vec3::zero();

    selected_positions.for_each(world, |position| {
        count += 1;
        sum += position.0;
    });

    average_selected_position.0 = if count != 0 {
        Some(sum / count as f32)
    } else {
        None
    };
}

#[system]
pub fn calculate_average_selected_end_position(
    #[resource] average_selected_end_position: &mut AverageSelectedEndPosition,
    selected_positions: &mut Query<(&Position, Option<&MovingTo>), SelectedFilter>,
    world: &legion::world::SubWorld,
) {
    let mut count = 0;
    let mut sum = Vec3::zero();

    selected_positions.iter(world)
        .map(|(position, moving_to)| {
            moving_to.and_then(|moving_to| moving_to.0.last().cloned()).unwrap_or(position.0)
        })
        .for_each(|position| {
            count += 1;
            sum += position;
        });

        average_selected_end_position.0 = if count != 0 {
        Some(sum / count as f32)
    } else {
        None
    };
}
