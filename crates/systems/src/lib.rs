use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;
use ultraviolet::Vec3;

mod combat;
mod rendering;
mod steering;

pub use combat::*;
pub use rendering::*;
pub use steering::*;

pub fn update_ship_rotation_matrix(
    mut query: Query<(&Rotation, &mut RotationMatrix, &ModelId), Changed<Rotation>>,
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
            if keyboard_state.shift && selected.get(entity).is_ok() {
                commands.entity(entity).remove::<Selected>();
            } else {
                commands.entity(entity).insert(Selected);
            }
        }
    }
}

pub fn handle_left_drag(
    mouse_state: Res<MouseState>,
    dimensions: Res<Dimensions>,
    perspective_view: Res<PerspectiveView>,
    query: Query<(Entity, &Position), With<ModelId>>,
    selected: Query<Entity, With<Selected>>,
    mut commands: Commands,
    keyboard_state: Res<KeyboardState>,
) {
    if let Some(start) = mouse_state.left_state.was_dragged() {
        let frustum = SelectionFrustum::new_from_onscreen_box(
            start.min_by_component(mouse_state.position),
            start.max_by_component(mouse_state.position),
            dimensions.width,
            dimensions.height,
            perspective_view.perspective_view_with_far_plane.inversed(),
        );

        if !keyboard_state.shift {
            selected.for_each(|entity| {
                commands.entity(entity).remove::<Selected>();
            });
        }

        query.for_each(|(entity, pos)| {
            if frustum.contains_point(pos.0) {
                commands.entity(entity).insert(Selected);
            }
        })
    }
}

type SelectedFriendly = (With<Selected>, With<Friendly>);

pub fn handle_right_clicks(
    mut commands: Commands,
    selected: Query<Entity, SelectedFriendly>,
    mouse_button: Res<MouseState>,
    average_selected_position: Res<AverageSelectedPosition>,
    mut mouse_mode: ResMut<MouseMode>,
    ray_plane_point: Res<RayPlanePoint>,
    ship_under_cursor: Res<ShipUnderCursor>,
    enemies: Query<&Enemy>,
    can_carry: Query<&Carrying>,
    can_attack: Query<Entity, (SelectedFriendly, With<CanAttack>)>,
    can_be_carried: Query<Entity, (SelectedFriendly, With<CanBeCarried>)>,
) {
    if mouse_button.right_state.was_clicked() {
        match ship_under_cursor.0 {
            Some(target_entity) => {
                if enemies.get(target_entity).is_ok() {
                    can_attack.for_each(|entity| {
                        commands.entity(entity).insert(Command::Interact {
                            target: target_entity,
                            ty: InteractionType::Attack,
                        });
                    });
                } else if can_carry.get(target_entity).is_ok() {
                    can_be_carried.for_each(|entity| {
                        commands.entity(entity).insert(Command::Interact {
                            target: target_entity,
                            ty: InteractionType::BeCarriedBy,
                        });
                    });
                }

                *mouse_mode = MouseMode::Normal
            }
            None => {
                *mouse_mode = match *mouse_mode {
                    MouseMode::Normal => match average_selected_position.0 {
                        Some(avg) => MouseMode::Movement { plane_y: avg.y },
                        _ => MouseMode::Normal,
                    },
                    MouseMode::Movement { .. } => {
                        if let Some(point) = ray_plane_point.0 {
                            selected.for_each(|entity| {
                                commands.entity(entity).insert(Command::MoveTo(point));
                            });
                        }

                        MouseMode::Normal
                    }
                };
            }
        }
    }
}

#[profiling::function]
pub fn set_rotation_from_moving_to(
    mut query: Query<(&Position, &Velocity, &mut Rotation), Changed<Velocity>>,
) {
    query.for_each_mut(|(position, velocity, mut rotation)| {
        if velocity.0 != Vec3::zero() {
            let delta = velocity.0;
            let xz_movement = ultraviolet::Vec2::new(delta.x, delta.z).mag();

            rotation.0 = ultraviolet::Rotor3::from_rotation_xz(-delta.x.atan2(delta.z))
                * ultraviolet::Rotor3::from_rotation_yz(-delta.y.atan2(xz_movement));
        }
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
            .y_plane_intersection(plane_y)
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
            commands.entity(entity).remove::<Command>();
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
    friendly_following: Query<&Position, (With<CameraFollowing>, With<Friendly>)>,
) {
    // If any friendly units are being followed, follow only friendly units.
    // This prevents problems where a whole bunch of units and a single asteroid
    // are selected and it messes with the average position.
    let avg = if friendly_following.iter().next().is_some() {
        average(friendly_following.iter().map(|pos| pos.0))
    } else {
        average(following.iter().map(|pos| pos.0))
    };

    if let Some(avg) = avg {
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

pub fn update_projectiles(mut query: Query<&mut Projectile>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|mut projectile| {
        projectile.update(delta_time.0);
    })
}

pub fn expand_explosions(mut query: Query<&mut Scale, With<Expands>>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|mut scale| {
        scale.0 += delta_time.0 * 1.5;
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

#[profiling::function]
pub fn set_world_space_bounding_box(
    mut query: Query<
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

pub fn spin(mut query: Query<(&mut Spin, &mut Rotation)>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|(mut spin, mut rotation)| {
        spin.update_angle(delta_time.0);
        rotation.0 = spin.as_rotor();
    });
}

fn get_scale(scale: Option<&Scale>) -> f32 {
    scale.map(|scale| scale.0).unwrap_or(1.0)
}

pub fn calculate_average_selected_position(
    mut average_selected_position: ResMut<AverageSelectedPosition>,
    selected_positions: Query<&Position, SelectedFriendly>,
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

pub fn apply_velocity(
    mut query: Query<(&mut Position, &Velocity)>,
    delta_time: Res<DeltaTime>,
    paused: Res<Paused>,
) {
    if paused.0 {
        return;
    }
    query.for_each_mut(|(mut position, velocity)| {
        position.0 += velocity.0 * delta_time.0;
    });
}

pub fn count_selected(
    friendly: Query<&ModelId, (With<Selected>, With<Friendly>)>,
    neutral: Query<&ModelId, (With<Selected>, Without<Friendly>, Without<Enemy>)>,
    enemy: Query<&ModelId, (With<Selected>, With<Enemy>)>,
    mut glyph_brush: ResMut<GlyphBrush>,
) {
    let mut section = glyph_brush::OwnedSection::default();

    let print = |mut section: glyph_brush::OwnedSection,
                 prefix,
                 colour,
                 counts: [u32; Models::COUNT]| {
        for i in 0..Models::COUNT {
            let count = counts[i];

            if count > 0 {
                section = section.add_text(glyph_brush::OwnedText::new(prefix).with_color(colour));

                section = section.add_text(
                    glyph_brush::OwnedText::new(&format!(" {:?}s: {}\n", Models::ARRAY[i], count))
                        .with_color([1.0; 4]),
                );
            }
        }

        section
    };

    section = print(
        section,
        "Friendly",
        [0.25, 1.0, 0.25, 1.0],
        count(friendly.iter()),
    );
    section = print(
        section,
        "Neutral",
        [0.25, 0.25, 1.0, 1.0],
        count(neutral.iter()),
    );
    section = print(
        section,
        "Enemy",
        [1.0, 0.25, 0.25, 1.0],
        count(enemy.iter()),
    );

    glyph_brush.queue(&section);
}

fn count<'a>(iter: impl Iterator<Item = &'a ModelId>) -> [u32; Models::COUNT] {
    let mut counts = [0; Models::COUNT];

    for model in iter {
        counts[*model as usize] += 1;
    }

    counts
}
