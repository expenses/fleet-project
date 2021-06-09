use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use ultraviolet::Vec3;

pub fn find_next_carrier<'a>(
    pos: Vec3,
    queue: &mut CommandQueue,
    carriers: impl Iterator<Item = (Entity, &'a Position)>,
) {
    let carrier = carriers
        .map(|(entity, new_pos)| {
            let dist_sq = (pos - new_pos.0).mag_sq();
            (entity, dist_sq)
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((entity, _)) = carrier {
        queue.0.push_front(Command::Interact {
            target: entity,
            ty: InteractionType::BeCarriedBy,
            range_sq: 0.0,
        });
    }
}

pub fn find_next_asteroid(
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
