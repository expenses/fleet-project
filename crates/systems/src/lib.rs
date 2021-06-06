// These are unavoidable when using an ecs really
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;
use components_and_resources::utils::*;
use ultraviolet::Vec3;

mod combat;
mod people;
mod rendering;
mod resource_management;
mod steering;

pub use combat::*;
pub use people::*;
pub use rendering::*;
pub use resource_management::*;
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
    query: Query<
        (
            Entity,
            &WorldSpaceBoundingBox,
            &ModelId,
            &Position,
            &RotationMatrix,
            Option<&Scale>,
        ),
        With<Selectable>,
    >,
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
    unit_buttons: Res<UnitButtons>,
    selected_button: Res<SelectedButton>,
    button_selection: Query<(Entity, &ModelId, Option<&Friendly>, Option<&Enemy>)>,
) {
    if mouse_button.left_state.was_clicked() {
        if let Some(button_index) = selected_button.0 {
            if let Some((button_model, button_status)) = unit_buttons.0.get(button_index) {
                let is_being_carried =
                    matches!(button_status, UnitStatus::Friendly { carried: true });
                // can't handle this case yet
                if is_being_carried {
                    return;
                }
                button_selection.for_each(|(entity, model_id, friendly, enemy)| {
                    let matches = model_id == button_model
                        && button_status
                            == &UnitStatus::from_bools(friendly.is_some(), enemy.is_some(), false);

                    if !matches ^ keyboard_state.shift {
                        commands.entity(entity).remove::<Selected>();
                    }
                });
            }
            return;
        }

        if matches!(*mouse_mode, MouseMode::Movement { .. }) {
            *mouse_mode = MouseMode::Normal;
            return;
        }

        if !keyboard_state.shift {
            selected.for_each(|entity| {
                commands.entity(entity).remove::<Selected>();
            });
        }

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
    query: Query<(Entity, &Position), (With<ModelId>, With<Selectable>)>,
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
    mut query_set: QuerySet<(
        Query<&mut CommandQueue, SelectedFriendly>,
        Query<&mut CommandQueue, (SelectedFriendly, With<CanAttack>)>,
        Query<&mut CommandQueue, (SelectedFriendly, With<CanBeCarried>)>,
        Query<&mut CommandQueue, (SelectedFriendly, With<CanMine>)>,
    )>,
    enemies: Query<&Enemy>,
    mouse_button: Res<MouseState>,
    average_selected_position: Res<AverageSelectedPosition>,
    mut mouse_mode: ResMut<MouseMode>,
    ray_plane_point: Res<RayPlanePoint>,
    ship_under_cursor: Res<ShipUnderCursor>,
    can_carry: Query<&Carrying>,
    can_be_mined: Query<&Scale, With<CanBeMined>>,
    keyboard_state: Res<KeyboardState>,
) {
    if mouse_button.right_state.was_clicked() {
        match ship_under_cursor.0 {
            Some(target_entity) => {
                if enemies.get(target_entity).is_ok() {
                    query_set.q1_mut().for_each_mut(|mut queue| {
                        if !keyboard_state.shift {
                            queue.0.clear();
                        }
                        queue.0.push_back(Command::Interact {
                            target: target_entity,
                            ty: InteractionType::Attack,
                            range_sq: 0.0,
                        });
                    });
                } else if can_carry.get(target_entity).is_ok() {
                    query_set.q2_mut().for_each_mut(|mut queue| {
                        if !keyboard_state.shift {
                            queue.0.clear();
                        }
                        queue.0.push_back(Command::Interact {
                            target: target_entity,
                            ty: InteractionType::BeCarriedBy,
                            range_sq: 0.0,
                        });
                    });
                } else if let Ok(scale) = can_be_mined.get(target_entity) {
                    query_set.q3_mut().for_each_mut(|mut queue| {
                        if !keyboard_state.shift {
                            queue.0.clear();
                        }
                        queue.0.push_back(Command::Interact {
                            target: target_entity,
                            ty: InteractionType::Mine,
                            range_sq: scale.range_sq(),
                        });
                    });
                }

                *mouse_mode = MouseMode::Normal
            }
            None => {
                *mouse_mode = match *mouse_mode {
                    MouseMode::Normal => match average_selected_position.0 {
                        Some(avg) => MouseMode::Movement {
                            plane_y: avg.y,
                            ty: MoveType::Normal,
                        },
                        _ => MouseMode::Normal,
                    },
                    MouseMode::Movement { ty, .. } => {
                        if let Some(point) = ray_plane_point.0 {
                            query_set.q0_mut().for_each_mut(|mut queue| {
                                queue.0.clear();
                                queue.0.push_back(Command::MoveTo { point, ty });
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
pub fn set_rotation_from_velocity(mut query: Query<(&Velocity, &mut Rotation), Changed<Velocity>>) {
    query.for_each_mut(|(velocity, mut rotation)| {
        if velocity.0 != Vec3::zero() {
            let xz_movement = ultraviolet::Vec2::new(velocity.0.x, velocity.0.z).mag();

            rotation.0 = ultraviolet::Rotor3::from_rotation_xz(-velocity.0.x.atan2(velocity.0.z))
                * ultraviolet::Rotor3::from_rotation_yz(-velocity.0.y.atan2(xz_movement));
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
        MouseMode::Movement { plane_y, .. } => ray
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
    mut selected_moving: Query<&mut CommandQueue, With<Selected>>,
    mut commands: Commands,
    keyboard_state: Res<KeyboardState>,
    mut paused: ResMut<Paused>,
    mut carrying: Query<(&Position, &mut Carrying), SelectedFriendly>,
    mut rng: ResMut<SmallRng>,
    average_selected_position: Res<AverageSelectedPosition>,
    mut mouse_mode: ResMut<MouseMode>,
) {
    if keyboard_state.stop.0 {
        selected_moving.for_each_mut(|mut queue| {
            queue.0.clear();
        });
    }

    if keyboard_state.pause.0 {
        paused.0 = !paused.0;
    }

    if keyboard_state.unload.0 {
        carrying.for_each_mut(|(pos, mut carrying)| {
            unload(pos.0, &mut carrying, &mut *rng, &mut commands);
        })
    }

    if keyboard_state.escape.0 {
        *mouse_mode = MouseMode::Normal;
    }

    if keyboard_state.attack_move.0 {
        match *mouse_mode {
            MouseMode::Movement { ref mut ty, .. } => *ty = MoveType::Attack,
            _ => {
                if let Some(avg) = average_selected_position.0 {
                    *mouse_mode = MouseMode::Movement {
                        plane_y: avg.y,
                        ty: MoveType::Attack,
                    };
                }
            }
        }
    }
}

pub fn handle_destruction(
    mut query: Query<(
        Entity,
        &Position,
        &Health,
        Option<&mut Carrying>,
        Option<&OnBoard>,
    )>,
    mut rng: ResMut<SmallRng>,
    mut commands: Commands,
    total_time: Res<TotalTime>,
) {
    query.for_each_mut(|(entity, pos, health, carrying, on_board)| {
        if health.0 <= 0.0 {
            if let Some(mut carrying) = carrying {
                unload(pos.0, &mut carrying, &mut *rng, &mut commands);
            }

            commands.entity(entity).despawn();

            if let Some(on_board) = on_board {
                for &entity in on_board.0.iter() {
                    commands.entity(entity).despawn();
                }
            }

            spawn_explosion(pos.0, total_time.0, &mut commands);
        }
    })
}

fn spawn_explosion(pos: Vec3, total_time: f32, commands: &mut Commands) {
    commands.spawn_bundle((
        Position(pos),
        RotationMatrix::default(),
        ModelId::Explosion,
        Scale(0.0),
        AliveUntil(total_time + 2.5),
        Expands,
    ));
}

fn unload(pos: Vec3, carrying: &mut Carrying, rng: &mut SmallRng, commands: &mut Commands) {
    carrying.0.drain(..).for_each(|entity| {
        commands
            .entity(entity)
            .insert(Position(pos))
            .insert(Command::MoveTo {
                point: pos + uniform_sphere_distribution(rng) * 5.0,
                ty: MoveType::Attack,
            });
    })
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

type SelectedUncarried = (With<Selected>, With<Position>);

pub fn count_selected(
    friendly: Query<&ModelId, (SelectedUncarried, With<Friendly>)>,
    neutral: Query<&ModelId, (SelectedUncarried, Without<Friendly>, Without<Enemy>)>,
    enemy: Query<&ModelId, (SelectedUncarried, With<Enemy>)>,
    mut glyph_brush: ResMut<GlyphBrush>,
    friendly_carrying: Query<&Carrying, (SelectedUncarried, With<Friendly>)>,
    all_models: Query<&ModelId>,
    mut buttons: ResMut<UnitButtons>,
) {
    let mut section = glyph_brush::OwnedSection::default();
    buttons.0.clear();

    let mut print = |mut section: glyph_brush::OwnedSection,
                     status: UnitStatus,
                     colour,
                     counts: [u32; Models::COUNT]| {
        for model_id in Models::ARRAY.iter().cloned() {
            let i = model_id as usize;
            let count = counts[i];

            if count > 0 {
                buttons.0.push((model_id, status));
                section = section
                    .add_text(glyph_brush::OwnedText::new(status.to_str()).with_color(colour));

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
        UnitStatus::Friendly { carried: false },
        [0.25, 1.0, 0.25, 1.0],
        count(friendly.iter()),
    );
    section = print(
        section,
        UnitStatus::Friendly { carried: true },
        [0.25, 1.0, 0.25, 1.0],
        count(
            friendly_carrying
                .iter()
                .flat_map(|carrying| &carrying.0)
                .filter_map(|&entity| all_models.get(entity).ok()),
        ),
    );
    section = print(
        section,
        UnitStatus::Neutral,
        [0.25, 0.25, 1.0, 1.0],
        count(neutral.iter()),
    );
    section = print(
        section,
        UnitStatus::Enemy,
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

pub fn set_selected_button(
    buttons: Res<UnitButtons>,
    mut selected_button: ResMut<SelectedButton>,
    mouse_state: Res<MouseState>,
) {
    if mouse_state.position.x > UnitButtons::BUTTON_WIDTH {
        selected_button.0 = None;
        return;
    }

    let index = mouse_state.position.y / UnitButtons::LINE_HEIGHT;

    let index = index as usize;

    selected_button.0 = if index < buttons.0.len() {
        Some(index)
    } else {
        None
    };
}
