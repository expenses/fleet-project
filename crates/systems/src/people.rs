use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

pub fn repair_ships(
    mut query: Query<(Entity, &OnBoard, Option<&Carrying>)>,
    mut health: Query<&mut Health>,
    engineers: Query<&Engineer>,
    delta_time: Res<DeltaTime>,
) {
    query.for_each_mut(|(entity, on_board, carrying)| {
        let mut health_increase_pool = on_board
            .0
            .iter()
            .filter(|&&person_entity| engineers.get(person_entity).is_ok())
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

pub fn perform_research(
    on_board: Query<(&OnBoard, Option<&ResearchMultiplier>), With<Friendly>>,
    researchers: Query<&Researcher>,
    delta_time: Res<DeltaTime>,
    mut global_research: ResMut<GlobalResearch>,
) {
    const BASE_RESEARCH_SPEED: f32 = 0.1;

    on_board.for_each(|(on_board, research_multiplier)| {
        let research_increase = on_board
            .0
            .iter()
            .filter(|&&person_entity| researchers.get(person_entity).is_ok())
            .count() as f32
            * delta_time.0
            * research_multiplier.map(|mul| mul.0).unwrap_or(1.0)
            * BASE_RESEARCH_SPEED;

        global_research.0 += research_increase;
    })
}
