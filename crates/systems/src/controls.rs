use crate::find_functions::find_next_carrier;
use crate::{average, get_scale, unload, unload_of_type, SelectedFriendly, UnloadParams};
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::formations::Formation;
use components_and_resources::resources::*;
use components_and_resources::utils::compare_floats;
use ultraviolet::Vec3;

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
                .find_with_owned_stack(
                    move |bbox| ray.bounding_box_intersection(bbox).is_some(),
                    Vec::with_capacity(10),
                )
                .filter_map(move |triangle| ray.triangle_intersection(triangle))
                // We need to multiply t by scale here as the time of impact is calculated on an unscaled model
                .map(move |t| (entity, t * scale))
        })
        .min_by(|&(_, a), &(_, b)| compare_floats(a, b))
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
    mut carrying: Query<(Entity, &Position, &mut Carrying), SelectedFriendly>,
    mut movement: Query<(&mut Velocity, &mut CommandQueue)>,
    models: Query<&ModelId>,
    mut rng: ResMut<SmallRng>,
    total_time: Res<TotalTime>,
) {
    if !mouse_button.left_state.was_clicked() {
        return;
    }

    if let Some(button_index) = selected_button.0 {
        if let Some((button_model, button_status)) = unit_buttons.0.get(button_index) {
            let is_being_carried = matches!(button_status, UnitStatus::Friendly { carried: true });
            if is_being_carried {
                carrying.for_each_mut(|(entity, pos, mut carrying)| {
                    unload_of_type(
                        UnloadParams {
                            entity,
                            pos: pos.0,
                            carrying: &mut carrying,
                            rng: &mut rng,
                            total_time: total_time.0,
                            commands: &mut commands,
                            movement: &mut movement,
                            selected: true,
                        },
                        &models,
                        *button_model,
                    );
                });
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
        Query<(&Position, &mut CommandQueue), SelectedFriendly>,
        Query<&mut CommandQueue, (SelectedFriendly, With<CanAttack>)>,
        Query<&mut CommandQueue, (SelectedFriendly, With<CanBeCarried>)>,
        Query<&mut CommandQueue, (SelectedFriendly, With<CanMine>)>,
    )>,
    selected_models: Query<&ModelId, (SelectedFriendly, With<Position>, With<CommandQueue>)>,
    enemies: Query<&Enemy>,
    mouse_button: Res<MouseState>,
    average_selected_position: Res<AverageSelectedPosition>,
    mut mouse_mode: ResMut<MouseMode>,
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
                        point_on_plane: Vec3::new(0.0, avg.y, 0.0),
                        ty: MoveType::Normal,
                    },
                    _ => MouseMode::Normal,
                },
                MouseMode::Movement { ty, point_on_plane } => {
                    if let Some(avg) = average_selected_position.0 {
                        let mut count = 0;
                        let mut all_fighters = true;

                        selected_models.for_each(|&model_id| {
                            count += 1;
                            all_fighters &= model_id == ModelId::Fighter;
                        });

                        let mut formation = if count == 1 {
                            Formation::at_point(point_on_plane, count)
                        } else if all_fighters {
                            Formation::fighter_screen(
                                point_on_plane,
                                (point_on_plane - avg).normalized(),
                                count,
                                5.0,
                            )
                        } else {
                            Formation::in_sphere(point_on_plane, count)
                        };

                        query_set.q0_mut().for_each_mut(|(pos, mut queue)| {
                            queue.0.clear();
                            if let Some(point) = formation.choose_position(pos.0) {
                                queue.0.push_back(Command::MoveTo { point, ty });
                            }
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
    mouse_state.right_state.update(delta_time.0, 0.1);
    mouse_state.middle_state.update(delta_time.0, 0.0);
}

pub fn update_ray_plane_point(
    ray: Res<Ray>,
    mut mouse_mode: ResMut<MouseMode>,
    keyboard_state: Res<KeyboardState>,
) {
    if let MouseMode::Movement {
        ref mut point_on_plane,
        ..
    } = &mut *mouse_mode
    {
        if !keyboard_state.shift {
            if let Some(point) = ray
                .y_plane_intersection(point_on_plane.y)
                .map(|t| ray.get_intersection_point(t))
            {
                point_on_plane.x = point.x;
                point_on_plane.z = point.z;
            }
        }
    }
}

pub fn move_camera(
    kbd: Res<KeyboardState>,
    orbit: Res<Orbit>,
    mouse: Res<MouseState>,
    dimensions: Res<Dimensions>,
    mut camera: ResMut<Camera>,
    currently_following: Query<Entity, With<CameraFollowing>>,
    mut commands: Commands,
) {
    let keyboard_control = camera.control(
        &orbit,
        kbd.camera_forwards,
        kbd.camera_back,
        kbd.camera_left,
        kbd.camera_right,
    );

    let edge_of_screen_control = camera.control(
        &orbit,
        mouse.position.y < 10.0,
        mouse.position.y > dimensions.height as f32 - 10.0,
        mouse.position.x < 10.0,
        mouse.position.x > dimensions.width as f32 - 10.0,
    );

    if keyboard_control || edge_of_screen_control {
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
    mut global_minerals: ResMut<GlobalMinerals>,
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
            unload(UnloadParams {
                entity,
                pos: pos.0,
                carrying: &mut carrying,
                rng: &mut *rng,
                total_time: total_time.0,
                commands: &mut commands,
                movement: &mut query_set.q1_mut(),
                selected: true,
            });
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
                        point_on_plane: Vec3::new(0.0, avg.y, 0.0),
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

    let build_ship_type = if keyboard_state.build_fighter.0 {
        Some(ShipType::Fighter)
    } else if keyboard_state.build_miner.0 {
        Some(ShipType::Miner)
    } else if keyboard_state.build_carrier.0 {
        Some(ShipType::Carrier)
    } else {
        None
    };

    if let Some(build_ship_type) = build_ship_type {
        let cost = build_ship_type.build_cost();
        if cost <= global_minerals.0 {
            global_minerals.0 -= cost;

            let best_queue = build_queues
                .iter_mut()
                .map(|queue| (queue.queue_length(total_time.0), queue))
                .min_by(|&(a, _), &(b, _)| compare_floats(a, b));

            if let Some((_, mut queue)) = best_queue {
                queue.push(build_ship_type, total_time.0);
            }
        }
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
