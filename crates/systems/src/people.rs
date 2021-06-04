use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

pub fn repair_ships(
    mut query: Query<(&mut Health, &MaxHealth, &OnBoard)>,
    people: Query<&PersonType>,
    delta_time: Res<DeltaTime>,
) {
    query.for_each_mut(|(mut health, max_health, on_board)| {
        let health_increase = on_board
            .0
            .iter()
            .filter_map(|&person_entity| people.get(person_entity).ok())
            .filter(|person_ty| matches!(person_ty, PersonType::Engineer))
            .count() as f32
            * delta_time.0;
        health.0 = (health.0 + health_increase).min(max_health.0);
    })
}
