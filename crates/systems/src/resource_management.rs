use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;
use ultraviolet::Vec3;

pub fn mine(
    mut query: Query<(&Position, &MaxSpeed, &mut CommandQueue, &mut StoredMinerals)>,
    mut targets: Query<(&Position, &mut CanBeMined)>,
    new_targets: Query<(Entity, &Position, &Scale), With<CanBeMined>>,
    carriers: Query<(Entity, &Position), With<Carrying>>,
    delta_time: Res<DeltaTime>,
    mut commands: Commands,
) {
    query.for_each_mut(|(pos, max_speed, mut queue, mut stored_minerals)| {
        if let Some(Command::Interact {
            target,
            ty: InteractionType::Mine,
            range_sq,
        }) = queue.0.front()
        {
            if stored_minerals.stored >= stored_minerals.capacity {
                queue.0.pop_front();
                find_next_carrier(pos.0, &mut queue, &carriers);
                find_next_asteroid(pos.0, &mut queue, &new_targets);
            } else if let Ok((target_pos, mut can_be_mined)) = targets.get_mut(*target) {
                let max_force = max_speed.max_force();
                let within_range = (pos.0 - target_pos.0).mag_sq() < range_sq + max_force;

                if within_range {
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
                    find_next_carrier(pos.0, &mut queue, &carriers);
                } else {
                    find_next_asteroid(pos.0, &mut queue, &new_targets);
                }
            }
        }
    })
}

fn find_next_carrier(
    pos: Vec3,
    queue: &mut CommandQueue,
    carriers: &Query<(Entity, &Position), With<Carrying>>,
) {
    let carrier = carriers
        .iter()
        .map(|(entity, new_pos)| {
            let dist_sq = (pos - new_pos.0).mag_sq();
            (entity, dist_sq)
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((entity, _)) = carrier {
        queue.0.push_back(Command::Interact {
            target: entity,
            ty: InteractionType::BeCarriedBy,
            range_sq: 0.0,
        });
    }
}

fn find_next_asteroid(
    pos: Vec3,
    queue: &mut CommandQueue,
    new_targets: &Query<(Entity, &Position, &Scale), With<CanBeMined>>,
) {
    let new_target = new_targets
        .iter()
        .map(|(entity, new_pos, scale)| {
            let dist_sq = (pos - new_pos.0).mag_sq();
            (entity, dist_sq, scale)
        })
        .min_by(|(_, a, _), (_, b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((entity, _, scale)) = new_target {
        queue.0.push_back(Command::Interact {
            target: entity,
            ty: InteractionType::Mine,
            range_sq: scale.range_sq(),
        });
    }
}

pub fn build_ships<Side: Default + Send + Sync + 'static>(
    mut query: Query<(&Position, &mut BuildQueue), With<Side>>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
    mut rng: ResMut<SmallRng>,
) {
    query.for_each_mut(|(pos, mut build_queue)| {
        if let Some(built_ship) = build_queue.advance(total_time.0) {
            let entity = spawn_ship::<Side>(built_ship, pos.0, &mut commands);

            let mut velocity = Velocity(Vec3::zero());
            let mut command_queue = CommandQueue::default();

            crate::unload_single(
                pos.0,
                entity,
                &mut rng,
                total_time.0,
                Some((&mut velocity, &mut command_queue)),
                &mut commands,
            );

            commands
                .entity(entity)
                .insert_bundle((velocity, command_queue));
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
        .insert_bundle(base_ship_components(pos, Vec::new()))
        .insert(Side::default());

    match ship {
        ShipType::Fighter => {
            spawner.insert_bundle(fighter_components(0.0));
        }
        ShipType::Miner => {
            spawner.insert_bundle(miner_components());
        }
        ShipType::Carrier => {
            spawner.insert_bundle(carrier_components(BuildQueue::default()));
        }
    }

    spawner.id()
}
