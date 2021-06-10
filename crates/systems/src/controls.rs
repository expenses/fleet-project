use crate::find_functions::find_next_carrier;
use crate::{average, get_scale, unload, SelectedFriendly};
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

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
    if !mouse_button.left_state.was_clicked() {
        return;
    }

    if let Some(button_index) = selected_button.0 {
        if let Some((button_model, button_status)) = unit_buttons.0.get(button_index) {
            let is_being_carried = matches!(button_status, UnitStatus::Friendly { carried: true });
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

pub fn handle_left_drag(
    mouse_state: Res<MouseState>,
    dimensions: Res<Dimensions>,
    perspective_view: Res<PerspectiveView>,
    query: Query<(Entity, &Position), (With<ModelId>, With<Selectable>)>,
    selected: Query<Entity, With<Selected>>,
    mut commands: Commands,
    keyboard_state: Res<KeyboardState>,
) {
    let start = match mouse_state.left_state.was_dragged() {
        Some(start) => start,
        None => return,
    };

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
    });
}

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
    if !mouse_button.right_state.was_clicked() {
        return;
    }

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
    mut query_set: QuerySet<(
        Query<&mut CommandQueue, SelectedFriendly>,
        Query<(&mut Velocity, &mut CommandQueue)>,
        Query<(&Position, &mut CommandQueue), (SelectedFriendly, With<CanBeCarried>)>,
    )>,
    mut commands: Commands,
    keyboard_state: Res<KeyboardState>,
    mut paused: ResMut<Paused>,
    mut carrying: Query<(Entity, &Position, &mut Carrying), SelectedFriendly>,
    mut rng: ResMut<SmallRng>,
    average_selected_position: Res<AverageSelectedPosition>,
    mut mouse_mode: ResMut<MouseMode>,
    total_time: Res<TotalTime>,
    carriers: Query<(Entity, &Position), (With<Carrying>, Without<CarrierFull>)>,
    mut build_queues: Query<&mut BuildQueue, SelectedFriendly>,
) {
    if keyboard_state.stop.0 {
        query_set.q0_mut().for_each_mut(|mut queue| {
            queue.0.clear();
        });
    }

    if keyboard_state.pause.0 {
        paused.0 = !paused.0;
    }

    if keyboard_state.unload.0 {
        carrying.for_each_mut(|(entity, pos, mut carrying)| {
            unload(
                entity,
                pos.0,
                &mut carrying,
                &mut *rng,
                total_time.0,
                &mut commands,
                &mut query_set.q1_mut(),
                true,
            );
        });

        build_queues.for_each_mut(|mut queue| {
            queue.stay_carried = false;
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

    if keyboard_state.load.0 {
        query_set.q2_mut().for_each_mut(|(pos, mut command_queue)| {
            command_queue.0.clear();
            find_next_carrier(pos.0, &mut command_queue, carriers.iter())
        });

        build_queues.for_each_mut(|mut queue| {
            queue.stay_carried = true;
        })
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
