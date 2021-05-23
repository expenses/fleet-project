use components_and_resources::components::*;
use components_and_resources::gpu_structs::{BackgroundVertex, CircleInstance, Instance};
use components_and_resources::resources::*;
use ultraviolet::Vec3;

use bevy_ecs::prelude::*;

pub fn update_ship_rotation_matrix(
    query: Query<(&Rotation, &mut RotationMatrix, &ModelId), Changed<Rotation>>,
    models: Res<Models>,
) {
    query.for_each_mut(|(rotation, mut rotation_matrix, model_id)| {
        let matrix = rotation.0.into_matrix();

        let model = models.get(*model_id);

        *rotation_matrix = RotationMatrix {
            matrix,
            reversed: rotation.0.reversed().into_matrix(),
            rotated_model_bounding_box: model.bounding_box.rotate(matrix),
        };
    });
}

pub fn clear_buffer<T: bytemuck::Pod + Send + Sync + 'static>(mut buffer: ResMut<GpuBuffer<T>>) {
    buffer.clear();
}

pub fn upload_buffer<T: bytemuck::Pod + Send + Sync + 'static>(
    mut buffer: ResMut<GpuBuffer<T>>,
    gpu_interface: Res<GpuInterface>,
) {
    buffer.upload(&gpu_interface.device, &gpu_interface.queue);
}

pub fn clear_ship_buffer(mut buffer: ResMut<ShipBuffer>) {
    buffer.clear();
}

pub fn upload_ship_buffer(mut buffer: ResMut<ShipBuffer>, gpu_interface: Res<GpuInterface>) {
    buffer.upload(&gpu_interface.device, &gpu_interface.queue);
}

pub fn upload_instances(
    query: Query<(
        Entity,
        Option<&Selected>,
        &Position,
        &RotationMatrix,
        &ModelId,
        Option<&Scale>,
        Option<&Friendly>,
        Option<&Enemy>,
    )>,
    ship_under_cursor: Res<ShipUnderCursor>,
    mut ship_buffer: ResMut<ShipBuffer>,
) {
    query.for_each(
        |(entity, selected, position, rotation_matrix, model_id, scale, friendly, enemy)| {
            let base_colour = if friendly.is_some() {
                Vec3::unit_y()
            } else if enemy.is_some() {
                Vec3::unit_x()
            } else {
                Vec3::unit_z()
            };

            let colour = if ship_under_cursor.0 == Some(entity) {
                base_colour
            } else if selected.is_some() {
                base_colour * 0.5
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
        },
    );
}

pub fn find_ship_under_cursor(
    query: Query<(
        Entity,
        &WorldSpaceBoundingBox,
        &ModelId,
        &Position,
        &RotationMatrix,
        Option<&Scale>,
    )>,
    ray: Res<Ray>,
    models: Res<Models>,
    mut ship_under_cursor: ResMut<ShipUnderCursor>,
) {
    ship_under_cursor.0 = query
        .iter()
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
        .map(|(entity, _)| entity);
}

pub fn debug_find_ship_under_cursor(
    query: Query<(
        &WorldSpaceBoundingBox,
        &ModelId,
        &Position,
        &RotationMatrix,
        Option<&Scale>,
    )>,
    ray: Res<Ray>,
    models: Res<Models>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
) {
    if let Some((tri, _, position, rotation, scale)) = query
        .iter()
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

pub fn update_ray(
    dimensions: Res<Dimensions>,
    orbit: Res<Orbit>,
    perspective_view: Res<PerspectiveView>,
    mouse_state: Res<MouseState>,
    mut ray: ResMut<Ray>,
    camera: Res<Camera>,
) {
    *ray = Ray::new_from_screen(
        mouse_state.position,
        dimensions.width,
        dimensions.height,
        orbit.as_vector() + camera.center,
        perspective_view.perspective.inversed(),
        perspective_view.view.inversed(),
    );
}

type SelectedFilter = (With<Selected>, With<Friendly>);

pub fn handle_left_click(
    mut commands: Commands,
    selected: Query<Entity, With<Selected>>,
    mouse_button: Res<MouseState>,
    ship_under_cursor: Res<ShipUnderCursor>,
    mut mouse_mode: ResMut<MouseMode>,
    keyboard_state: Res<KeyboardState>,
) {
    if mouse_button.left_state.was_clicked() {
        if !keyboard_state.shift {
            selected.for_each(|entity| {
                commands.entity(entity).remove::<Selected>();
            });
        }

        *mouse_mode = MouseMode::Normal;

        if let Some(entity) = ship_under_cursor.0 {
            println!("Selecting {:?}", entity);
            commands.entity(entity).insert(Selected);
        }
    }
}

pub fn handle_right_clicks(
    mut commands: Commands,
    selected: Query<Entity, SelectedFilter>,
    mouse_button: Res<MouseState>,
    average_selected_position: Res<AverageSelectedPosition>,
    mut mouse_mode: ResMut<MouseMode>,
    ray_plane_point: Res<RayPlanePoint>,
) {
    if mouse_button.right_state.was_clicked() {
        *mouse_mode = match *mouse_mode {
            MouseMode::Normal => match average_selected_position.0 {
                Some(avg) => MouseMode::Movement { plane_y: avg.y },
                _ => MouseMode::Normal,
            },
            MouseMode::Movement { .. } => {
                if let Some(point) = ray_plane_point.0 {
                    selected.for_each(|entity| {
                        commands.entity(entity).insert(MovingTo(point));
                    });
                }

                MouseMode::Normal
            }
        };
    }
}

pub fn move_ships(
    query: Query<(Entity, &mut Position, &MovingTo, &MaxSpeed)>,
    mut commands: Commands,
    delta_time: Res<DeltaTime>,
) {
    query.for_each_mut(|(entity, mut position, moving_to, max_speed)| {
        let delta = moving_to.0 - position.0;
        let distance = delta.mag();
        let speed = max_speed.0 * delta_time.0;

        if distance < speed {
            position.0 = moving_to.0;
            commands.entity(entity).remove::<MovingTo>();
        } else {
            position.0 += delta / distance * speed;
        }
    })
}

pub fn set_rotation_from_moving_to(
    query: Query<(&Position, &MovingTo, &mut Rotation), Changed<MovingTo>>,
) {
    query.for_each_mut(|(position, moving_to, mut rotation)| {
        let delta = moving_to.0 - position.0;
        let xz_movement = ultraviolet::Vec2::new(delta.x, delta.z).mag();

        rotation.0 = ultraviolet::Rotor3::from_rotation_xz(-delta.x.atan2(delta.z))
            * ultraviolet::Rotor3::from_rotation_yz(-delta.y.atan2(xz_movement));
    })
}

pub fn update_mouse_state(mut mouse_state: ResMut<MouseState>, delta_time: Res<DeltaTime>) {
    mouse_state.left_state.update(delta_time.0, 0.1);
    mouse_state.right_state.update(delta_time.0, 0.075);
}

pub fn update_ray_plane_point(
    ray: Res<Ray>,
    mouse_mode: Res<MouseMode>,
    mut ray_plane_point: ResMut<RayPlanePoint>,
) {
    ray_plane_point.0 = match *mouse_mode {
        MouseMode::Movement { plane_y } => ray
            .plane_intersection(plane_y)
            .map(|t| ray.get_intersection_point(t)),
        MouseMode::Normal => None,
    };
}

pub fn move_camera(
    keyboard_state: Res<KeyboardState>,
    orbit: Res<Orbit>,
    mut camera: ResMut<Camera>,
    currently_following: Query<Entity, With<CameraFollowing>>,
    mut commands: Commands,
) {
    if keyboard_state.move_camera(&mut camera, &orbit) {
        currently_following.for_each(|entity| {
            commands.entity(entity).remove::<CameraFollowing>();
        });
    }
}

pub fn handle_keys(
    selected_moving: Query<Entity, With<Selected>>,
    mut commands: Commands,
    keyboard_state: Res<KeyboardState>,
    mut paused: ResMut<Paused>,
) {
    if keyboard_state.stop.0 {
        selected_moving.for_each(|entity| {
            commands.entity(entity).remove::<MovingTo>();
        });
    }

    if keyboard_state.pause.0 {
        paused.0 = !paused.0;
    }
}

pub fn update_keyboard_state(mut keyboard_state: ResMut<KeyboardState>) {
    keyboard_state.update();
}

pub fn set_camera_following(
    keyboard_state: Res<KeyboardState>,
    selected: Query<Entity, With<Selected>>,
    currently_following: Query<Entity, With<CameraFollowing>>,
    mut commands: Commands,
) {
    if keyboard_state.center_camera.0 {
        // If we deselect everything and press 'center camera while following
        // something, it makes the most sense to keep following that thing.
        if selected.iter().next().is_some() {
            currently_following.for_each(|entity| {
                commands.entity(entity).remove::<CameraFollowing>();
            });

            selected.for_each(|entity| {
                commands.entity(entity).insert(CameraFollowing);
            });
        }
    }
}

pub fn move_camera_around_following(
    mut camera: ResMut<Camera>,
    mut perspective_view: ResMut<PerspectiveView>,
    orbit: Res<Orbit>,
    following: Query<&Position, With<CameraFollowing>>,
) {
    if let Some(avg) = average(following.iter().map(|pos| pos.0)) {
        camera.center = avg;
    }

    perspective_view.set_view(orbit.as_vector(), camera.center);
}

pub fn spawn_projectiles(
    ray: Res<Ray>,
    keyboard_state: Res<KeyboardState>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
) {
    if keyboard_state.fire {
        commands.spawn_bundle((
            Projectile::new(&ray, 10.0),
            AliveUntil(total_time.0 + 30.0),
            Friendly,
        ));
    }
}

pub fn render_projectiles(
    query: Query<&Projectile>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
) {
    query.for_each(|projectile| {
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
    })
}

pub fn update_projectiles(query: Query<&mut Projectile>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|mut projectile| {
        projectile.update(delta_time.0);
    })
}

pub fn collide_projectiles<Side>(
    projectiles: Query<(Entity, &Projectile), With<Side>>,
    ships: Query<
        (
            Entity,
            &WorldSpaceBoundingBox,
            &Position,
            &RotationMatrix,
            &ModelId,
            Option<&Scale>,
        ),
        Without<Side>,
    >,
    models: Res<Models>,
    delta_time: Res<DeltaTime>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
    indestructible: Query<&Indestructible>,
) where
    Side: Send + Sync + 'static,
{
    projectiles.for_each(|(entity, projectile)| {
        let bounding_box = projectile.bounding_box(delta_time.0);

        let first_hit = ships
            .iter()
            .filter(|(_, ship_bounding_box, ..)| bounding_box.intersects(ship_bounding_box.0))
            .flat_map(|(ship_entity, _, position, rotation, model_id, scale)| {
                let scale = get_scale(scale);

                let ray = projectile
                    .as_limited_ray(delta_time.0)
                    .centered_around_transform(position.0, rotation.reversed, scale);

                models
                    .get(*model_id)
                    .acceleration_tree
                    .locate_with_selection_function_with_data(ray)
                    .map(move |(_, scaled_t)| (ship_entity, scaled_t))
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((ship_entity, t)) = first_hit {
            let position = projectile.get_intersection_point(t);

            commands.entity(entity).despawn();
            if indestructible.get(ship_entity).is_err() {
                commands.entity(ship_entity).despawn();
            }
            commands.spawn_bundle((
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

pub fn expand_explosions(query: Query<&mut Scale, With<Expands>>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|mut scale| {
        scale.0 += delta_time.0;
    });
}

pub fn kill_temporary(
    query: Query<(Entity, &AliveUntil)>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
) {
    query.for_each(|(entity, alive_until)| {
        if total_time.0 > alive_until.0 {
            commands.entity(entity).despawn();
        }
    })
}

pub fn increase_total_time(mut total_time: ResMut<TotalTime>, delta_time: Res<DeltaTime>) {
    total_time.0 += delta_time.0;
}

// We cache these because it's 6 f32 adds and that adds time to bounding box checks
// if we do them per ray.
type SetWorldBBoxFilter = Or<(Changed<Position>, Changed<RotationMatrix>, Changed<Scale>)>;

pub fn set_world_space_bounding_box(
    query: Query<
        (
            &mut WorldSpaceBoundingBox,
            &Position,
            &RotationMatrix,
            Option<&Scale>,
        ),
        SetWorldBBoxFilter,
    >,
) {
    query.for_each_mut(|(mut bounding_box, position, rotation, scale)| {
        bounding_box.0 = (rotation.rotated_model_bounding_box * get_scale(scale)) + position.0;
    });
}

pub fn spin(query: Query<(&mut Spin, &mut Rotation)>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|(mut spin, mut rotation)| {
        spin.update_angle(delta_time.0);
        rotation.0 = spin.as_rotor();
    });
}

fn get_scale(scale: Option<&Scale>) -> f32 {
    scale.map(|scale| scale.0).unwrap_or(1.0)
}

pub fn render_movement_circle(
    mut circle_instances: ResMut<GpuBuffer<CircleInstance>>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
    ray_plane_point: Res<RayPlanePoint>,
    average_selected_position: Res<AverageSelectedPosition>,
) {
    if let (Some(avg), Some(point)) = (average_selected_position.0, ray_plane_point.0) {
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

pub fn calculate_average_selected_position(
    mut average_selected_position: ResMut<AverageSelectedPosition>,
    selected_positions: Query<&Position, SelectedFilter>,
) {
    average_selected_position.0 = average(selected_positions.iter().map(|pos| pos.0));
}

fn average(positions: impl Iterator<Item = Vec3>) -> Option<Vec3> {
    let mut count = 0;
    let mut sum = Vec3::zero();

    for position in positions {
        count += 1;
        sum += position;
    }

    if count != 0 {
        Some(sum / count as f32)
    } else {
        None
    }
}
