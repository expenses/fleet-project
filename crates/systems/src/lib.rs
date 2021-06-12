// These are unavoidable when using an ecs really
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::resources::*;
use components_and_resources::utils::*;
use std::array::IntoIter;
use std::ops::{Deref, DerefMut};
use ultraviolet::{Vec2, Vec3};

mod combat;
mod controls;
mod find_functions;
mod people;
mod rendering;
mod resource_management;
mod steering;

pub use combat::*;
pub use controls::*;
pub use people::*;
pub use rendering::*;
pub use resource_management::*;
pub use steering::*;

type SelectedFriendly = (With<Selected>, With<Friendly>);

pub fn update_ship_rotation_matrix(
    mut query: Query<(&Rotation, &mut RotationMatrix, &ModelId), Changed<Rotation>>,
    models: Res<Models>,
) {
    query.for_each_mut(|(rotation, mut rotation_matrix, model_id)| {
        let matrix = rotation.0.into_matrix();

        let model = models.get(*model_id);

        *rotation_matrix = RotationMatrix {
            matrix,
            reversed: rotation.0.reversed().into_matrix(),
            rotated_model_bounding_box: model.bounding_box.rotate(matrix),
        };
    });
}

pub fn clear_buffer<T: bytemuck::Pod + Send + Sync + 'static>(mut buffer: ResMut<GpuBuffer<T>>) {
    buffer.clear();
}

pub fn upload_buffer<T: bytemuck::Pod + Send + Sync + 'static>(
    mut buffer: ResMut<GpuBuffer<T>>,
    gpu_interface: Res<GpuInterface>,
) {
    buffer.upload(&gpu_interface.device, &gpu_interface.queue);
}

pub fn clear_ship_buffer(mut buffer: ResMut<ShipBuffer>) {
    buffer.clear();
}

pub fn upload_ship_buffer(
    mut buffer: ResMut<ShipBuffer>,
    gpu_interface: Res<GpuInterface>,
    models: Res<Models>,
) {
    buffer.upload(&gpu_interface.device, &gpu_interface.queue, &models);
}

#[profiling::function]
pub fn set_rotation_from_velocity(mut query: Query<(&Velocity, &mut Rotation), Changed<Velocity>>) {
    query.for_each_mut(|(velocity, mut rotation)| {
        if velocity.0 != Vec3::zero() {
            rotation.0 = rotation_from_facing(velocity.0);
        }
    })
}

pub fn handle_destruction(
    mut query: Query<(
        Entity,
        &Position,
        &Health,
        Option<&mut Carrying>,
        Option<&OnBoard>,
        Option<&TlasIndex>,
        Option<&Selected>,
    )>,
    mut rng: ResMut<SmallRng>,
    mut commands: Commands,
    total_time: Res<TotalTime>,
    mut movement: Query<(&mut Velocity, &mut CommandQueue)>,
    mut tlas: ResMut<TopLevelAccelerationStructure>,
) {
    query.for_each_mut(
        |(entity, pos, health, carrying, on_board, tlas_index, selected)| {
            if health.current > 0.0 {
                return;
            }

            if let Some(mut carrying) = carrying {
                unload(
                    entity,
                    pos.0,
                    &mut carrying,
                    &mut *rng,
                    total_time.0,
                    &mut commands,
                    &mut movement,
                    selected.is_some(),
                );
            }

            commands.entity(entity).despawn();

            if let Some(on_board) = on_board {
                for &entity in on_board.0.iter() {
                    commands.entity(entity).despawn();
                }
            }

            if let Some(tlas_index) = tlas_index {
                tlas.remove(tlas_index.index);
            }

            spawn_explosion(pos.0, total_time.0, &mut *rng, &mut commands);
        },
    )
}

fn spawn_explosion(pos: Vec3, total_time: f32, rng: &mut SmallRng, commands: &mut Commands) {
    commands.spawn_bundle((
        Position(pos),
        RotationMatrix::random_for_rendering_only(rng),
        ModelId::Explosion,
        Scale(0.0),
        AliveUntil(total_time + 2.5),
        Expands,
    ));
}

fn unload(
    entity: Entity,
    pos: Vec3,
    carrying: &mut Carrying,
    rng: &mut SmallRng,
    total_time: f32,
    commands: &mut Commands,
    movement: &mut Query<(&mut Velocity, &mut CommandQueue)>,
    selected: bool,
) {
    commands.entity(entity).remove::<CarrierFull>();

    carrying.drain().for_each(|entity| {
        unload_single(
            pos,
            entity,
            rng,
            total_time,
            movement.get_mut(entity).ok(),
            commands,
            selected,
        );
    })
}

fn unload_single<V, M>(
    pos: Vec3,
    entity: Entity,
    rng: &mut SmallRng,
    total_time: f32,
    movement: Option<(V, M)>,
    commands: &mut Commands,
    select: bool,
) where
    V: Deref<Target = Velocity> + DerefMut,
    M: Deref<Target = CommandQueue> + DerefMut,
{
    let mut entity_commands = commands.entity(entity);

    entity_commands
        .insert(Position(pos))
        .insert(Unloading::new(total_time));

    if select {
        entity_commands.insert(Selected);
    }

    if let Some((mut velocity, mut queue)) = movement {
        velocity.0 = Vec3::zero();

        queue.0.push_front(Command::MoveTo {
            point: pos + uniform_sphere_distribution(rng) * 5.0,
            ty: MoveType::Attack,
        })
    }
}

pub fn update_projectiles(mut query: Query<&mut Projectile>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|mut projectile| {
        projectile.update(delta_time.0);
    })
}

pub fn expand_explosions(mut query: Query<&mut Scale, With<Expands>>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|mut scale| {
        scale.0 += delta_time.0 * 1.5;
    });
}

pub fn kill_temporary(
    query: Query<(Entity, &AliveUntil)>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
) {
    query.for_each(|(entity, alive_until)| {
        if total_time.0 > alive_until.0 {
            commands.entity(entity).despawn();
        }
    })
}

pub fn increase_total_time(mut total_time: ResMut<TotalTime>, delta_time: Res<DeltaTime>) {
    total_time.0 += delta_time.0;
}

// We cache these because it's 6 f32 adds and that adds time to bounding box checks
// if we do them per ray.
type SetWorldBBoxFilter = Or<(Changed<Position>, Changed<RotationMatrix>, Changed<Scale>)>;

#[profiling::function]
pub fn set_world_space_bounding_box(
    mut query: Query<
        (
            &mut WorldSpaceBoundingBox,
            &Position,
            &RotationMatrix,
            Option<&Scale>,
        ),
        SetWorldBBoxFilter,
    >,
) {
    query.for_each_mut(|(mut bounding_box, position, rotation, scale)| {
        bounding_box.0 = (rotation.rotated_model_bounding_box * get_scale(scale)) + position.0;
    });
}

pub fn spin(mut query: Query<(&mut Spin, &mut Rotation)>, delta_time: Res<DeltaTime>) {
    query.for_each_mut(|(mut spin, mut rotation)| {
        spin.update_angle(delta_time.0);
        rotation.0 = spin.as_rotor();
    });
}

fn get_scale(scale: Option<&Scale>) -> f32 {
    scale.map(|scale| scale.0).unwrap_or(1.0)
}

pub fn calculate_average_selected_position(
    mut average_selected_position: ResMut<AverageSelectedPosition>,
    selected_positions: Query<&Position, SelectedFriendly>,
) {
    average_selected_position.0 = average(selected_positions.iter().map(|pos| pos.0));
}

fn average(positions: impl Iterator<Item = Vec3>) -> Option<Vec3> {
    let mut count = 0;
    let mut sum = Vec3::zero();

    for position in positions {
        count += 1;
        sum += position;
    }

    if count != 0 {
        Some(sum / count as f32)
    } else {
        None
    }
}

pub fn apply_velocity(
    mut query: Query<(&mut Position, &Velocity)>,
    delta_time: Res<DeltaTime>,
    paused: Res<Paused>,
) {
    if paused.0 {
        return;
    }
    query.for_each_mut(|(mut position, velocity)| {
        position.0 += velocity.0 * delta_time.0;
    });
}

#[derive(Default)]
pub struct MultiString {
    cache_string: String,
    lengths_and_colours: Vec<(usize, [f32; 4])>,
    glyph_section: glyph_brush::Section<'static, glyph_brush::Extra>,
}

impl MultiString {
    pub fn set_position(&mut self, position: Vec2) {
        self.glyph_section.screen_position = position.into();
    }

    pub fn push(&mut self, args: std::fmt::Arguments, colour: [f32; 4]) {
        use std::fmt::Write;

        let start = self.cache_string.len();
        let _ = self.cache_string.write_fmt(args);
        let end = self.cache_string.len();

        let length = end - start;

        match self.lengths_and_colours.last_mut() {
            Some((last_length, last_colour)) if *last_colour == colour => {
                *last_length += length;
            }
            _ => {
                self.lengths_and_colours.push((length, colour));
            }
        }
    }

    pub fn queue_section(&mut self, glyph_brush: &mut GlyphBrush) {
        let mut offset = 0;

        for (length, colour) in &self.lengths_and_colours {
            let string = &self.cache_string[offset..offset + length];
            offset += length;

            // Use a transmute to change the lifetime of the string to be static.
            // This is VERY naughty but as far as I can tell is safe because the string
            // only needs to last until it is queued in the glyph brush.
            let string: &'static str = unsafe { std::mem::transmute(string) };
            self.glyph_section
                .text
                .push(glyph_brush::Text::new(string).with_color(*colour));
        }

        if !self.glyph_section.text.is_empty() {
            glyph_brush.queue(&self.glyph_section);
        }

        self.glyph_section.text.clear();
        self.lengths_and_colours.clear();
        self.cache_string.clear();
    }
}

type SelectedUncarried = (With<Selected>, With<Position>);

pub fn count_selected(
    friendly: Query<&ModelId, (SelectedUncarried, With<Friendly>)>,
    neutral: Query<&ModelId, (SelectedUncarried, Without<Friendly>, Without<Enemy>)>,
    enemy: Query<&ModelId, (SelectedUncarried, With<Enemy>)>,
    mut glyph_brush: ResMut<GlyphBrush>,
    friendly_carrying: Query<&Carrying, (SelectedUncarried, With<Friendly>)>,
    all_models: Query<&ModelId>,
    mut buttons: ResMut<UnitButtons>,
    global_minerals: Res<GlobalMinerals>,
    mut string_cache: Local<MultiString>,
) {
    buttons.0.clear();

    string_cache.push(
        format_args!("Global Minerals: {}\n", global_minerals.0),
        [1.0; 4],
    );

    let mut print = |status: UnitStatus, colour, counts: [u32; Models::COUNT]| {
        for model_id in IntoIter::new(Models::ARRAY) {
            let i = model_id as usize;
            let count = counts[i];

            if count > 0 {
                buttons.0.push((model_id, status));
                string_cache.push(format_args!("{}", status.to_str()), colour);

                string_cache.push(
                    format_args!(" {:?}s: {}\n", Models::ARRAY[i], count),
                    [1.0; 4],
                );
            }
        }
    };

    print(
        UnitStatus::Friendly { carried: false },
        [0.25, 1.0, 0.25, 1.0],
        count(friendly.iter()),
    );
    print(
        UnitStatus::Friendly { carried: true },
        [0.25, 1.0, 0.25, 1.0],
        count(
            friendly_carrying
                .iter()
                .flat_map(|carrying| carrying.iter())
                .filter_map(|entity| all_models.get(entity).ok()),
        ),
    );
    print(
        UnitStatus::Neutral,
        [0.25, 0.25, 1.0, 1.0],
        count(neutral.iter()),
    );
    print(
        UnitStatus::Enemy,
        [1.0, 0.25, 0.25, 1.0],
        count(enemy.iter()),
    );

    string_cache.queue_section(&mut glyph_brush);
}

fn count<'a>(iter: impl Iterator<Item = &'a ModelId>) -> [u32; Models::COUNT] {
    let mut counts = [0; Models::COUNT];

    for model in iter {
        counts[*model as usize] += 1;
    }

    counts
}

pub fn set_selected_button(
    buttons: Res<UnitButtons>,
    mut selected_button: ResMut<SelectedButton>,
    mouse_state: Res<MouseState>,
) {
    if mouse_state.position.x > UnitButtons::BUTTON_WIDTH {
        selected_button.0 = None;
        return;
    }

    let index = mouse_state.position.y / UnitButtons::LINE_HEIGHT;

    let index = index as isize - UnitButtons::UI_LINES;

    selected_button.0 = if index < buttons.0.len() as isize && index >= 0 {
        Some(index as usize)
    } else {
        None
    };
}

#[profiling::function]
pub fn update_tlas(
    mut tlas: ResMut<DynamicBvh<Entity>>,
    // We need to filter to ships that have a `Position` here to prevent carried ships being re-added to
    // the TLAS.
    mut query: Query<(Entity, &WorldSpaceBoundingBox, Option<&mut TlasIndex>), With<Position>>,
    mut commands: Commands,
) {
    query.for_each_mut(|(entity, bbox, tlas_index)| {
        let padded_bounding_box = bbox.0.expand(0.5);

        match tlas_index {
            Some(mut tlas_index) => {
                if !tlas_index.padded_bounding_box.contains(bbox.0) {
                    tlas.modify_bounding_box_and_refit(tlas_index.index, padded_bounding_box);
                    tlas_index.padded_bounding_box = padded_bounding_box;
                }
            }
            None => {
                let index = tlas.insert(entity, padded_bounding_box);
                commands.entity(entity).insert(TlasIndex {
                    index,
                    padded_bounding_box,
                });
            }
        }
    });
}

pub fn remove_unloading(
    query: Query<(Entity, &Unloading)>,
    total_time: Res<TotalTime>,
    mut commands: Commands,
) {
    query.for_each(|(entity, unloading)| {
        if unloading.until <= total_time.0 {
            commands.entity(entity).remove::<Unloading>();
        }
    })
}

pub fn debug_watch(
    query: Query<(Option<&Position>, Option<&RotationMatrix>, Option<&ModelId>), With<DebugWatch>>,
) {
    query.for_each(|(pos, matrix, model_id)| {
        dbg!(pos, matrix, model_id);
    })
}
