use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::gpu_structs::BackgroundVertex;
use components_and_resources::resources::*;
use ultraviolet::Vec3;

mod primitives;

#[profiling::function]
pub fn run_steering(
    mut query: Query<(
        Entity,
        &Position,
        &Velocity,
        &MaxSpeed,
        Option<&Command>,
        Option<&Evading>,
        &mut StagingVelocity,
    )>,
    boids: Query<(&Position, &Velocity, &MaxSpeed)>,
    mut commands: Commands,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
) {
    query.for_each_mut(|(entity, pos, vel, max_speed, command, evading, mut sv)| {
        let mut steering = Vec3::zero();
        let boid = to_boid((pos, vel, max_speed));
        let max_force = max_speed.0 / 10.0;

        if let Some(command) = command {
            match *command {
                Command::Attack(target_entity) => {
                    if let Ok(target_boid) = boids.get(target_entity).map(to_boid) {
                        // Because ships are constantly turning, the predicted
                        // point of contact for a ship far away varies a lot, resulting
                        // in an annoying visual wobble. So we disable leading here.
                        // We should fix this someother how though.
                        let lead_factor = 0.0;

                        let force = boid.persue(target_boid, lead_factor);

                        /*lines_buffer.stage(&[
                            BackgroundVertex {
                                position: pos.0,
                                colour: Vec3::unit_x(),
                            },
                            BackgroundVertex {
                                position: pos.0 + force,
                                colour: Vec3::unit_x(),
                            },
                        ]);*/

                        steering += force;
                    } else {
                        commands.entity(entity).remove::<Command>();
                    }
                }
                Command::MoveTo(point) => {
                    let force = boid.seek(point);

                    steering += force;

                    if (boid.pos - point).mag_sq() < max_force {
                        commands.entity(entity).remove::<Command>();
                    }
                }
            }
        }

        if let Some(&Evading(evading_entity)) = evading {
            if let Ok(evading_boid) = boids.get(evading_entity).map(to_boid) {
                let force = boid.evade(evading_boid) * 0.5;

                /*lines_buffer.stage(&[
                    BackgroundVertex {
                        position: pos.0,
                        colour: Vec3::unit_y(),
                    },
                    BackgroundVertex {
                        position: pos.0 + force,
                        colour: Vec3::unit_y(),
                    },
                ]);*/

                steering += force;
            } else {
                commands.entity(entity).remove::<Evading>();
            }
        }

        /*
        {
            let force = boid.avoidance(boids.iter(world).map(to_boid)) * 0.5;

            steering += force;

            lines_buffer.stage(&[
                BackgroundVertex {
                    position: pos.0,
                    colour: Vec3::new(1.0, 0.5, 0.0)
                },
                BackgroundVertex {
                    position: pos.0 + force,
                    colour: Vec3::new(1.0, 0.5, 0.0)
                }
            ]);
        }
        */

        if steering == Vec3::zero() {
            steering = -boid.vel;
        }

        let steering = truncate(steering, max_force);

        /*lines_buffer.stage(&[
            BackgroundVertex {
                position: pos.0,
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: pos.0 + steering,
                colour: Vec3::unit_z(),
            },
        ]);*/

        *sv = StagingVelocity(truncate(vel.0 + steering, max_speed.0));
    })
}

fn to_boid((pos, vel, max_speed): (&Position, &Velocity, &MaxSpeed)) -> primitives::Boid {
    primitives::Boid {
        pos: pos.0,
        vel: vel.0,
        max_vel: max_speed.0,
        radius_sq: 4.0_f32.powi(2),
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

pub fn apply_staging_velocity(
    mut query: Query<(&mut Velocity, &StagingVelocity)>,
    paused: Res<Paused>,
) {
    if paused.0 {
        return;
    }
    query.for_each_mut(|(mut velocity, staging_velocity)| {
        velocity.0 = staging_velocity.0;
    });
}
