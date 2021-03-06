use crate::find_functions::*;
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;
use ultraviolet::Vec3;

mod primitives;

#[profiling::function]
pub fn run_persuit(
    mut query: Query<(
        Entity,
        &Position,
        &Velocity,
        &MaxSpeed,
        Option<&mut CommandQueue>,
        Option<&mut StoredMinerals>,
        &mut StagingPersuitForce,
        &TlasIndex,
        Option<&CanAttack>,
    )>,
    to_transfer: Query<&mut OnBoard>,
    boids: Query<(&Position, Option<&Velocity>, Option<&MaxSpeed>)>,
    mut commands: Commands,
    mut carrying: Query<&mut Carrying>,
    total_time: Res<TotalTime>,
    mut global_minerals: ResMut<GlobalMinerals>,
    mut tlas: ResMut<TopLevelAccelerationStructure>,
    carriers: Query<(Entity, &Position), (With<Carrying>, Without<CarrierFull>)>,
) {
    query.for_each_mut(|(entity, pos, vel, max_speed, queue, stored_minerals, mut staging_persuit_force, tlas_index, can_attack)| {
        let boid = to_boid(pos, vel, max_speed);
        let max_force = max_speed.max_force();

        let can_attack = can_attack.is_some();

        let mut queue = match queue {
            Some(queue) => queue,
            None => {
                staging_persuit_force.0 = Vec3::zero();
                return;
            }
        };

        let command = match queue.0.front().copied() {
            Some(command) => command,
            None => {
                staging_persuit_force.0 = Vec3::zero();
                return;
            }
        };

        match command {
            Command::Interact { target, ty, range_sq } => {
                let target_boid = match boids.get(target) {
                    Ok((p, v, ms)) => {
                        to_boid(p, &v.copied().unwrap_or_default(), &ms.copied().unwrap_or_default())
                    },
                    _ => {
                        queue.0.pop_front();
                        return;
                    }
                };

                // Because ships are constantly turning, the predicted
                // point of contact for a ship far away varies a lot, resulting
                // in an annoying visual wobble. So we disable leading here.
                // We should fix this someother how though.
                let lead_factor = 0.0;

                let within_range = (boid.pos - target_boid.pos).mag_sq() < range_sq + max_force;

                if !within_range {
                    staging_persuit_force.0 = boid.persue(target_boid, lead_factor);
                    return;
                }

                staging_persuit_force.0 = Vec3::zero();

                match ty {
                    InteractionType::BeCarriedBy => {
                        queue.0.pop_front();

                        let mut carrying = match carrying.get_mut(target) {
                            Ok(carrying) => carrying,
                            Err(err) => {
                                log::error!(
                                    "Entity {:?} tried to be carried by {:?} but {:?} cannot carry ships: {}",
                                    entity, target, target, err
                                );
                                return;
                            }
                        };

                        // If the carrier is full, the ship can't load into it
                        // and should look for another (non-full) one. If it's
                        // just docking to drop somethings off then that's fine though.
                        if carrying.is_full() && queue.0.is_empty() {
                            // Note: `redirect_ships_from_full_carriers` should redirect the ship
                            // before it comes to this, but this is just to make sure.
                            find_next_carrier(pos.0, &mut queue, carriers.iter());
                            return;
                        }

                        let mut entity_commands = commands.entity(entity);

                        if queue.0.is_empty() {
                            if !carrying.checked_push(entity, can_attack) {
                                log::error!("Failed to push to {:?}s carrying list", target);
                            }

                            tlas.remove(tlas_index.index);

                            entity_commands
                                .remove::<TlasIndex>()
                                .remove::<Position>()
                                .remove::<Selected>();
                        } else {
                            entity_commands.insert(Unloading::new(total_time.0));
                        }

                        if carrying.is_full() {
                            commands.entity(target).insert(CarrierFull);
                        }

                        let ship_to_transfer = unsafe {
                            to_transfer.get_unchecked(entity)
                        };

                        let carrier_to_transfer = unsafe {
                            to_transfer.get_unchecked(target)
                        };

                        if let (Ok(mut ship_people), Ok(mut carrier_people)) = (ship_to_transfer, carrier_to_transfer) {
                            carrier_people.0.append(&mut ship_people.0);
                        }

                        if let Some(mut stored_minerals) = stored_minerals {
                            global_minerals.0 += stored_minerals.stored;
                            stored_minerals.stored = 0.0;
                        }
                    },
                    InteractionType::Mine => {}
                    InteractionType::Attack => {}
                }
            }
            Command::MoveTo { point, .. } => {
                staging_persuit_force.0 = boid.seek(point);

                if (boid.pos - point).mag_sq() < max_force {
                    queue.0.pop_front();
                }
            }
        }
    })
}

#[profiling::function]
pub fn run_evasion(
    mut query: Query<(
        Entity,
        &Position,
        &Velocity,
        &MaxSpeed,
        Option<&Evading>,
        &CommandQueue,
        &mut StagingEvasionForce,
    )>,
    boids: Query<(&Position, &Velocity, &MaxSpeed)>,
    mut commands: Commands,
) {
    query.for_each_mut(
        |(entity, pos, vel, max_speed, evading, queue, mut staging_evasion_force)| {
            let should_evade = matches!(
                queue.0.front(),
                None | Some(Command::Interact {
                    ty: InteractionType::Attack,
                    ..
                })
            );

            if !should_evade {
                staging_evasion_force.0 = Vec3::zero();
                return;
            }

            let entity_to_evade = match evading {
                Some(&Evading(entity_to_evade)) => entity_to_evade,
                _ => {
                    staging_evasion_force.0 = Vec3::zero();
                    return;
                }
            };

            let evading_boid = match boids.get(entity_to_evade) {
                Ok((p, v, ms)) => to_boid(p, v, ms),
                _ => {
                    staging_evasion_force.0 = Vec3::zero();
                    commands.entity(entity).remove::<Evading>();
                    return;
                }
            };

            let boid = to_boid(pos, vel, max_speed);

            staging_evasion_force.0 = boid.flee(evading_boid.pos) * 0.5;
        },
    )
}

#[profiling::function]
pub fn run_avoidance(
    mut query: Query<(
        Entity,
        &Position,
        &Velocity,
        &MaxSpeed,
        Option<&CommandQueue>,
        &mut StagingAvoidanceForce,
        Option<&Carrying>,
    )>,
    boids: Query<(
        Option<&CommandQueue>,
        Option<&Unloading>,
        &Position,
        &Velocity,
        &MaxSpeed,
    )>,
    task_pool: Res<bevy_tasks::TaskPool>,
    bvh: Res<TopLevelAccelerationStructure>,
) {
    query.par_for_each_mut(
        &task_pool,
        8,
        |(entity, pos, vel, max_speed, queue, mut steering_avoidance_force, carrying)| {
            let boid = to_boid(pos, vel, max_speed);

            let max_radius = boid.radius_sq.sqrt().max(10.0);

            let bbox =
                BoundingBox::new(-Vec3::broadcast(max_radius), Vec3::broadcast(max_radius)) + pos.0;

            let get_be_carried_by_entity =
                |queue: Option<&CommandQueue>| match queue.and_then(|queue| queue.0.front()) {
                    Some(Command::Interact {
                        target,
                        ty: InteractionType::BeCarriedBy,
                        ..
                    }) => Some(*target),
                    _ => None,
                };

            let be_carried_by_entity = get_be_carried_by_entity(queue);

            let is_carrier = carrying.is_some();

            let mut find_stack = Vec::with_capacity(10);

            let iter = bvh
                .find(
                    |bounding_box| bbox.intersects(bounding_box),
                    &mut find_stack,
                )
                .filter_map(|&entity| {
                    boids
                        .get(entity)
                        .ok()
                        .map(|components| (entity, components))
                })
                .filter(|&(avoid_entity, (avoid_queue, unloading, ..))| {
                    let avoid_entity_carry_target = get_be_carried_by_entity(avoid_queue);
                    let boid_is_unloading = unloading.is_some();

                    Some(avoid_entity) != be_carried_by_entity
                        && avoid_entity_carry_target != Some(entity)
                        && !(is_carrier && boid_is_unloading)
                })
                .map(|(_, (.., p, v, ms))| to_boid(p, v, ms));

            steering_avoidance_force.0 = boid.avoidance(iter) * 0.1;
        },
    )
}

fn to_boid(pos: &Position, vel: &Velocity, max_speed: &MaxSpeed) -> primitives::Boid {
    primitives::Boid {
        pos: pos.0,
        vel: vel.0,
        max_vel: max_speed.0,
        radius_sq: 1.5_f32.powi(2),
    }
}

fn truncate(vec: Vec3, max: f32) -> Vec3 {
    let mag = vec.mag();
    let new_mag = mag.min(max);
    if new_mag == 0.0 {
        Vec3::zero()
    } else {
        vec / mag * new_mag
    }
}

#[profiling::function]
pub fn apply_staging_velocity(
    mut query: Query<(
        &mut Velocity,
        &MaxSpeed,
        &StagingPersuitForce,
        &StagingEvasionForce,
        &StagingAvoidanceForce,
    )>,
    paused: Res<Paused>,
) {
    if paused.0 {
        return;
    }
    query.for_each_mut(|(mut velocity, max_speed, persuit, evasion, avoidance)| {
        let max_force = max_speed.max_force();

        let mut steering = persuit.0 + evasion.0 + avoidance.0;

        if steering == Vec3::zero() {
            steering = -velocity.0;
        }

        let steering = truncate(steering, max_force);

        velocity.0 = truncate(velocity.0 + steering, max_speed.0);
    });
}
