use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

pub fn mine(
    mut query: Query<(&Position, &MaxSpeed, &mut CommandQueue, &mut StoredMinerals)>,
    mut targets: Query<(&Position, &mut CanBeMined)>,
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
            if let Ok((target_pos, mut can_be_mined)) = targets.get_mut(*target) {
                let max_force = max_speed.max_force();
                let within_range = (pos.0 - target_pos.0).mag_sq() < range_sq + max_force;

                if within_range {
                    let to_mine = delta_time.0;
                    let to_mine = to_mine.min(can_be_mined.minerals);
                    can_be_mined.minerals -= to_mine;

                    stored_minerals.0 += to_mine;

                    if to_mine == 0.0 {
                        commands.entity(*target).remove::<CanBeMined>();
                    }
                }
            } else {
                queue.0.pop_front();
            }
        }
    })
}
