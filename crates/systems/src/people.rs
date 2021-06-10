use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

pub fn repair_ships(
    mut query: Query<(Entity, &OnBoard, Option<&Carrying>)>,
    mut health: Query<&mut Health>,
    people: Query<&PersonType>,
    delta_time: Res<DeltaTime>,
) {
    query.for_each_mut(|(entity, on_board, carrying)| {
        let mut health_increase_pool = on_board
            .0
            .iter()
            .filter_map(|&person_entity| people.get(person_entity).ok())
            .filter(|person_ty| matches!(person_ty, PersonType::Engineer))
            .count() as f32
            * delta_time.0;

        if let Ok(mut health) = health.get_mut(entity) {
            let health_increase = health_increase_pool.min(health.max - health.current);

            health.current += health_increase;
            health_increase_pool -= health_increase;
        }

        if let Some(carrying) = carrying {
            for entity in carrying.iter() {
                if health_increase_pool == 0.0 {
                    break;
                }

                if let Ok(mut health) = health.get_mut(entity) {
                    let health_increase = health_increase_pool.min(health.max - health.current);

                    health.current += health_increase;
                    health_increase_pool -= health_increase;
                }
            }
        }
    })
}
