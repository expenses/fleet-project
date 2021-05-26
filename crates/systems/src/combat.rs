use crate::get_scale;
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

#[profiling::function]
pub fn collide_projectiles<Side>(
    projectiles: Query<(Entity, &Projectile), With<Side>>,
    ships: Query<
        (
            Entity,
            &WorldSpaceBoundingBox,
            &Position,
            &RotationMatrix,
            &ModelId,
            Option<&Scale>,
        ),
        Without<Side>,
    >,
    models: Res<Models>,
    delta_time: Res<DeltaTime>,
    total_time: Res<TotalTime>,
    commands: Commands,
    indestructible: Query<&Indestructible>,
    task_pool: Res<bevy_tasks::TaskPool>,
) where
    Side: Send + Sync + 'static,
{
    let commands = parking_lot::Mutex::new(commands);

    projectiles.par_for_each(&task_pool, 16, |(entity, projectile)| {
        let bounding_box = projectile.bounding_box(delta_time.0);

        let first_hit = ships
            .iter()
            .filter(|(_, ship_bounding_box, ..)| bounding_box.intersects(ship_bounding_box.0))
            .flat_map(|(ship_entity, _, position, rotation, model_id, scale)| {
                let scale = get_scale(scale);

                let ray = projectile
                    .as_limited_ray(delta_time.0)
                    .centered_around_transform(position.0, rotation.reversed, scale);

                models
                    .get(*model_id)
                    .acceleration_tree
                    .locate_with_selection_function_with_data(ray)
                    .map(move |(_, scaled_t)| (ship_entity, scaled_t))
            })
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((ship_entity, t)) = first_hit {
            let position = projectile.get_intersection_point(t);

            let mut commands = commands.lock();

            commands.entity(entity).despawn();
            if indestructible.get(ship_entity).is_err() {
                commands.entity(ship_entity).despawn();
            }
            commands.spawn_bundle((
                Position(position),
                RotationMatrix::default(),
                ModelId::Explosion,
                Scale(0.0),
                AliveUntil(total_time.0 + 2.5),
                Expands,
            ));
        }
    });
}

#[profiling::function]
pub fn choose_enemy_target<SideA, SideB>(
    query: Query<(Entity, &Position, &AgroRange), (With<SideA>, Without<Command>)>,
    candidates: Query<(Entity, &Position), (With<SideB>, With<Command>)>,
    mut commands: Commands,
) where
    SideA: Send + Sync + 'static,
    SideB: Send + Sync + 'static,
{
    query.for_each(|(entity, pos, agro_range)| {
        let agro_range_sq = agro_range.0 * agro_range.0;

        let target = candidates
            .iter()
            .filter_map(|(target_entity, target_pos)| {
                let dist_sq = (target_pos.0 - pos.0).mag_sq();

                if dist_sq < agro_range_sq {
                    Some((target_entity, dist_sq))
                } else {
                    None
                }
            })
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((target_entity, _)) = target {
            commands
                .entity(entity)
                .insert(Command::Attack(target_entity));
            commands.entity(target_entity).insert(Evading(entity));
        }
    });
}

pub fn spawn_projectile_from_ships<Side: Send + Sync + Default + 'static>(
    mut query: Query<(&Position, &Velocity, &mut RayCooldown, &Command), With<Side>>,
    delta_time: Res<DeltaTime>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
) {
    query.for_each_mut(|(pos, vel, mut ray_cooldown, command)| {
        ray_cooldown.0 = (ray_cooldown.0 - delta_time.0).max(0.0);

        if matches!(command, Command::Attack(_)) && ray_cooldown.0 == 0.0 {
            ray_cooldown.0 = 1.0;

            let ray = Ray::new(pos.0, vel.0);

            commands.spawn_bundle((
                Projectile::new(&ray, 100.0),
                AliveUntil(total_time.0 + 10.0),
                Side::default(),
            ));
        }
    })
}
