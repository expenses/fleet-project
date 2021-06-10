use crate::find_functions::*;
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::gpu_structs::LaserVertex;
use components_and_resources::resources::*;
use ultraviolet::Vec3;

pub fn mine(
    mut query: Query<(
        &Position,
        &MaxSpeed,
        &mut CommandQueue,
        &mut StoredMinerals,
        &mut Rotation,
    )>,
    mut targets: Query<(&Position, &mut CanBeMined)>,
    new_targets: Query<(Entity, &Position, &Scale), With<CanBeMined>>,
    carriers: Query<(Entity, &Position), With<Carrying>>,
    delta_time: Res<DeltaTime>,
    mut commands: Commands,
    mut lasers: ResMut<GpuBuffer<LaserVertex>>,
) {
    query.for_each_mut(
        |(pos, max_speed, mut queue, mut stored_minerals, mut rotation)| {
            let (target, range_sq) = match queue.0.front() {
                Some(Command::Interact {
                    target,
                    ty: InteractionType::Mine,
                    range_sq,
                }) => (target, range_sq),
                _ => return,
            };

            if stored_minerals.stored >= stored_minerals.capacity {
                queue.0.pop_front();
                find_next_carrier(pos.0, &mut queue, carriers.iter());
                find_next_asteroid(pos.0, &mut queue, &new_targets);
                return;
            }

            if let Ok((target_pos, mut can_be_mined)) = targets.get_mut(*target) {
                let max_force = max_speed.max_force();
                let vector = target_pos.0 - pos.0;
                let within_range = vector.mag_sq() < range_sq + max_force;

                if within_range {
                    rotation.0 = crate::rotation_from_facing(vector);

                    // This is not good in terms of 'seperation of concerns' but whatever
                    {
                        let laser_start = pos.0 + rotation.0 * Models::MINER_LASER_OFFSET;

                        lasers.stage(&[
                            LaserVertex {
                                position: laser_start,
                                colour: Vec3::unit_z(),
                            },
                            LaserVertex {
                                position: target_pos.0,
                                colour: Vec3::unit_x(),
                            },
                        ]);
                    }

                    let to_mine = delta_time.0;
                    let to_mine = to_mine
                        .min(can_be_mined.minerals)
                        .min(stored_minerals.capacity - stored_minerals.stored);
                    can_be_mined.minerals -= to_mine;

                    stored_minerals.stored += to_mine;

                    if to_mine == 0.0 {
                        commands.entity(*target).remove::<CanBeMined>();
                    }
                }
            } else {
                queue.0.pop_front();

                if new_targets.iter().next().is_none() {
                    find_next_carrier(pos.0, &mut queue, carriers.iter());
                } else {
                    find_next_asteroid(pos.0, &mut queue, &new_targets);
                }
            }
        },
    )
}

pub fn build_ships<Side: Default + Send + Sync + 'static>(
    mut query: Query<
        (
            &Position,
            &mut BuildQueue,
            Option<&Selected>,
            Option<&mut Carrying>,
        ),
        With<Side>,
    >,
    total_time: Res<TotalTime>,
    mut commands: Commands,
    mut rng: ResMut<SmallRng>,
) {
    query.for_each_mut(|(pos, mut build_queue, selected, carrying)| {
        if let Some(built_ship) = build_queue.advance(total_time.0) {
            let entity = spawn_ship::<Side>(built_ship, pos.0, &mut commands);

            if build_queue.stay_carried && built_ship != ShipType::Carrier {
                if let Some(mut carrying) = carrying {
                    if carrying.checked_push(entity, built_ship == ShipType::Fighter) {
                        commands.entity(entity).remove::<Position>();
                        return;
                    }
                }
            }

            let mut velocity = Velocity(Vec3::zero());
            let mut command_queue = CommandQueue::default();

            crate::unload_single(
                pos.0,
                entity,
                &mut rng,
                total_time.0,
                Some((&mut velocity, &mut command_queue)),
                &mut commands,
                selected.is_some(),
            );

            commands
                .entity(entity)
                .insert_bundle((velocity, command_queue));
        }
    })
}

pub fn redirect_ships_from_full_carriers(
    mut query: Query<&mut CommandQueue>,
    full_carriers: Query<&Position, With<CarrierFull>>,
    carriers_with_room: Query<(Entity, &Position), (With<Carrying>, Without<CarrierFull>)>,
) {
    query.for_each_mut(|mut queue| {
        let is_targetting_full_carrier_and_its_position = queue
            .0
            .front()
            .and_then(|command| match command {
                Command::Interact {
                    target,
                    ty: InteractionType::BeCarriedBy,
                    ..
                } => Some(target),
                _ => None,
            })
            .and_then(|&target| full_carriers.get(target).ok());

        // Note: we redirect to the closest carrier _to the carrier being targetted_,
        // not the ship we're redirecting. This is so the ships go to carriers in the same
        // region of space as opposed to being scattered all over the place.
        if let Some(target_pos) = is_targetting_full_carrier_and_its_position {
            queue.0.pop_front();
            find_next_carrier(target_pos.0, &mut queue, carriers_with_room.iter())
        }
    })
}

fn spawn_ship<Side: Default + Send + Sync + 'static>(
    ship: ShipType,
    pos: Vec3,
    commands: &mut Commands,
) -> Entity {
    let mut spawner = commands.spawn();

    spawner
        .insert_bundle(base_ship_components(pos))
        .insert(Side::default());

    match ship {
        ShipType::Fighter => {
            spawner.insert_bundle(fighter_components(0.0));
        }
        ShipType::Miner => {
            spawner.insert_bundle(miner_components());
        }
        ShipType::Carrier => {
            spawner.insert_bundle(carrier_components(BuildQueue::default(), Vec::new()));
        }
    }

    spawner.id()
}
