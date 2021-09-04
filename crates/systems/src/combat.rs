use super::{get_scale, spawn_explosion};
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;

#[profiling::function]
pub fn collide_projectiles<Side>(
    projectiles: Query<(Entity, &Projectile), With<Side>>,
    ships: Query<(&Position, &RotationMatrix, &ModelId, Option<&Scale>), Without<Side>>,
    models: Res<Models>,
    delta_time: Res<DeltaTime>,
    total_time: Res<TotalTime>,
    commands: Commands,
    health: Query<&mut Health>,
    task_pool: Res<bevy_tasks::TaskPool>,
    rng: ResMut<SmallRng>,
    bvh: Res<TopLevelAccelerationStructure>,
) where
    Side: Send + Sync + 'static,
{
    let on_hit_resources = parking_lot::Mutex::new((commands, health, rng));

    projectiles.par_for_each(&task_pool, 16, |(entity, projectile)| {
        let bounding_box = projectile.bounding_box(delta_time.0);

        let mut find_stack = Vec::with_capacity(10);

        let first_hit = bvh
            .find(
                |ship_bounding_box| bounding_box.intersects(ship_bounding_box),
                &mut find_stack,
            )
            .filter_map(|&entity| {
                ships
                    .get(entity)
                    .ok()
                    .map(|components| (entity, components))
            })
            .flat_map(|(ship_entity, (position, rotation, model_id, scale))| {
                let scale = get_scale(scale);

                let ray = projectile
                    .as_limited_ray(delta_time.0)
                    .centered_around_transform(position.0, rotation.reversed, scale);

                models
                    .get(*model_id)
                    .acceleration_tree
                    .find_with_owned_stack(
                        move |bbox| ray.bounding_box_intersection(bbox),
                        Vec::with_capacity(10),
                    )
                    .filter_map(move |triangle| ray.triangle_intersection(triangle))
                    .map(move |scaled_t| (ship_entity, scaled_t))
            })
            .max_by(|(_, a, ..), (_, b, ..)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((ship_entity, t)) = first_hit {
            let position = projectile.get_intersection_point(t);

            let mut lock_guard = on_hit_resources.lock();
            let (ref mut commands, ref mut health, ref mut rng) = &mut *lock_guard;

            commands.entity(entity).despawn();
            if let Ok(mut health) = health.get_mut(ship_entity) {
                health.current -= 10.0;
            }
            spawn_explosion(position, total_time.0, &mut *rng, commands);
        }
    });
}

#[profiling::function]
pub fn choose_enemy_target<SideA, SideB>(
    mut query: Query<
        (Entity, &Position, &AgroRange, &mut CommandQueue),
        (With<SideA>, With<CanAttack>),
    >,
    candidates: Query<(Entity, &Position), With<SideB>>,
    commands: Commands,
    task_pool: Res<bevy_tasks::TaskPool>,
) where
    SideA: Send + Sync + 'static,
    SideB: Send + Sync + 'static,
{
    let commands = parking_lot::Mutex::new(commands);

    query.par_for_each_mut(&task_pool, 8, |(entity, pos, agro_range, mut queue)| {
        match queue.0.front() {
            None
            | Some(Command::MoveTo {
                ty: MoveType::Attack,
                ..
            }) => {}
            _ => return,
        };

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
            queue.0.push_front(Command::Interact {
                target: target_entity,
                ty: InteractionType::Attack,
                range_sq: 0.0,
            });
            commands
                .lock()
                .entity(target_entity)
                .insert(Evading(entity));
        }
    });
}

pub fn spawn_projectile_from_ships<Side: Send + Sync + Default + 'static>(
    mut query: Query<
        (
            &Position,
            &Velocity,
            &mut RayCooldown,
            &CommandQueue,
            &AgroRange,
        ),
        With<Side>,
    >,
    positions: Query<&Position>,
    delta_time: Res<DeltaTime>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
) {
    query.for_each_mut(|(pos, vel, mut ray_cooldown, queue, agro_range)| {
        ray_cooldown.0 = (ray_cooldown.0 - delta_time.0).max(0.0);

        if ray_cooldown.0 != 0.0 {
            return;
        }

        let attack_target = match queue.0.front() {
            Some(Command::Interact {
                ty: InteractionType::Attack,
                target,
                ..
            }) => target,
            _ => return,
        };

        let agro_range_sq = agro_range.0 * agro_range.0;

        let in_range = match positions.get(*attack_target) {
            Ok(target_pos) => (pos.0 - target_pos.0).mag_sq() < agro_range_sq,
            _ => false,
        };

        if !in_range {
            return;
        }

        ray_cooldown.0 = 1.0;

        let ray = Ray::new(pos.0, vel.0.normalized());

        commands.spawn_bundle((
            Projectile::new(&ray, 200.0),
            AliveUntil(total_time.0 + 10.0),
            Side::default(),
        ));
    })
}
