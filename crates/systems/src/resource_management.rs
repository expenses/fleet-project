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

pub fn convert_minerals_to_fuel(
    mut query: Query<(&mut StoredMinerals, &mut StoredFuel)>,
    delta_time: Res<DeltaTime>,
) {
    query.for_each_mut(|(mut minerals, mut fuel)| {
        if minerals.stored > 0.0 {
            let to_convert = 0.5 * delta_time.0;
            let to_convert = to_convert.min(minerals.stored);

            minerals.stored -= to_convert;
            fuel.0 += to_convert;
        }
    })
}
