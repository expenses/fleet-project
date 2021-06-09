use super::get_scale;
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::gpu_structs::{
    BackgroundVertex, CircleInstance, Instance, LaserVertex, RangeInstance, Vertex2D,
};
use components_and_resources::resources::*;
use std::array::IntoIter;
use ultraviolet::{Vec2, Vec3, Vec4};

#[profiling::function]
pub fn render_model_instances(
    query: Query<(
        Entity,
        Option<&Selected>,
        &Position,
        &RotationMatrix,
        &ModelId,
        Option<&Scale>,
        Option<&Friendly>,
        Option<&Enemy>,
        Option<&CanBeMined>,
    )>,
    ship_under_cursor: Res<ShipUnderCursor>,
    mut ship_buffer: ResMut<ShipBuffer>,
    models: Res<Models>,
    misc_textures: Res<MiscTextures>,
) {
    query.for_each(
        |(
            entity,
            selected,
            position,
            rotation_matrix,
            model_id,
            scale,
            friendly,
            enemy,
            can_be_mined,
        )| {
            let base_colour = if friendly.is_some() {
                Vec3::unit_y()
            } else if enemy.is_some() {
                Vec3::unit_x()
            } else {
                Vec3::unit_z()
            };

            let colour = if ship_under_cursor.0 == Some(entity) {
                base_colour
            } else if selected.is_some() {
                base_colour * 0.5
            } else {
                Vec3::zero()
            };

            let model = models.get(*model_id);

            ship_buffer.stage(
                Instance {
                    translation: position.0,
                    rotation: rotation_matrix.matrix,
                    colour,
                    scale: get_scale(scale),
                    diffuse_texture: if *model_id == ModelId::Asteroid && can_be_mined.is_none() {
                        misc_textures.mined_out_asteroid
                    } else {
                        model.diffuse_texture
                    },
                    emissive_texture: model.emissive_texture,
                },
                *model_id as usize,
            );
        },
    );
}

pub fn debug_render_find_ship_under_cursor(
    query: Query<(
        &WorldSpaceBoundingBox,
        &ModelId,
        &Position,
        &RotationMatrix,
        Option<&Scale>,
    )>,
    ray: Res<Ray>,
    models: Res<Models>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
) {
    if let Some((tri, _, position, rotation, scale)) = query
        .iter()
        .filter(|(bounding_box, ..)| ray.bounding_box_intersection(bounding_box.0).is_some())
        .flat_map(|(_, model_id, position, rotation, scale)| {
            let scale = get_scale(scale);

            let ray = ray.centered_around_transform(position.0, rotation.reversed, scale);

            models
                .get(*model_id)
                .acceleration_tree
                .locate_with_selection_function_with_data(ray)
                .map(move |(tri, t)| (tri, t * scale, position, rotation, scale))
        })
        .min_by(|(_, a, ..), (_, b, ..)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    {
        lines_buffer.stage(&[
            BackgroundVertex {
                position: position.0 + rotation.matrix * tri.a * scale,
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_b_a) * scale,
                colour: Vec3::unit_y(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_b_a) * scale,
                colour: Vec3::unit_y(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_c_a) * scale,
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * (tri.a + tri.edge_c_a) * scale,
                colour: Vec3::unit_z(),
            },
            BackgroundVertex {
                position: position.0 + rotation.matrix * tri.a * scale,
                colour: Vec3::unit_x(),
            },
            /*
            BackgroundVertex {
                position: ray.get_intersection_point(t) - Vec3::broadcast(0.5),
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: ray.get_intersection_point(t) + Vec3::broadcast(0.5),
                colour: Vec3::unit_x(),
            },
            */
        ]);
    }
}

pub fn render_projectiles(
    query: Query<&Projectile>,
    mut lines_buffer: ResMut<GpuBuffer<LaserVertex>>,
) {
    query.for_each(|projectile| {
        let (start, end) = projectile.line_points(-0.1);

        let colour = Vec3::new(0.75, 0.0, 1.0) * 0.75;

        lines_buffer.stage(&[
            LaserVertex {
                position: start,
                colour,
            },
            LaserVertex {
                position: end,
                colour,
            },
        ]);
    })
}

pub fn render_movement_circle(
    mut circle_instances: ResMut<GpuBuffer<CircleInstance>>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
    ray_plane_point: Res<RayPlanePoint>,
    average_selected_position: Res<AverageSelectedPosition>,
    mouse_mode: Res<MouseMode>,
) {
    if let (Some(avg), Some(point), MouseMode::Movement { plane_y, ty }) =
        (average_selected_position.0, ray_plane_point.0, &*mouse_mode)
    {
        let mut circle_center = avg;
        circle_center.y = *plane_y;

        let scale = (point - circle_center).mag();

        let colour = match ty {
            MoveType::Normal => Vec3::unit_y(),
            MoveType::Attack => Vec3::unit_x(),
        };
        let colour_with_alpha = ultraviolet::Vec4::new(colour.x, colour.y, colour.z, 0.15);

        circle_instances.stage(&[CircleInstance {
            translation: circle_center,
            scale,
            colour: colour_with_alpha,
        }]);

        lines_buffer.stage(&[
            BackgroundVertex {
                position: avg,
                colour,
            },
            BackgroundVertex {
                position: point,
                colour,
            },
            BackgroundVertex {
                position: point,
                colour,
            },
            BackgroundVertex {
                position: circle_center,
                colour,
            },
            BackgroundVertex {
                position: circle_center,
                colour,
            },
            BackgroundVertex {
                position: avg,
                colour,
            },
        ])
    }
}

pub fn debug_render_targets(
    query: Query<(&Position, &CommandQueue), With<Selected>>,
    positions: Query<&Position>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
) {
    query.for_each(|(position, queue)| {
        let target_pos = match queue.0.front() {
            Some(Command::MoveTo { point, .. }) => Some(*point),
            Some(Command::Interact { target, .. }) => {
                positions.get(*target).ok().map(|position| position.0)
            }
            None => None,
        };

        if let Some(target_pos) = target_pos {
            lines_buffer.stage(&[
                BackgroundVertex {
                    position: position.0,
                    colour: Vec3::zero(),
                },
                BackgroundVertex {
                    position: target_pos,
                    colour: Vec3::one(),
                },
            ]);
        }
    })
}

pub fn render_agro_ranges(
    query: Query<(&Position, &AgroRange), (With<Friendly>, With<Selected>)>,
    mut ranges: ResMut<GpuBuffer<RangeInstance>>,
) {
    query.for_each(|(position, range)| {
        ranges.stage(&[RangeInstance {
            translation: position.0,
            scale: range.0,
            colour: Vec4::one(),
        }]);
    })
}

pub fn render_drag_box(
    mouse_state: Res<MouseState>,
    dimensions: Res<Dimensions>,
    mut lines_2d: ResMut<GpuBuffer<Vertex2D>>,
) {
    if let Some(start) = mouse_state.left_state.is_being_dragged() {
        let start = to_wgpu(start, &dimensions);
        let end = to_wgpu(mouse_state.position, &dimensions);

        lines_2d.stage(&[
            Vertex2D {
                pos: start,
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: Vec2::new(end.x, start.y),
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: Vec2::new(end.x, start.y),
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: end,
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: end,
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: Vec2::new(start.x, end.y),
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: Vec2::new(start.x, end.y),
                colour: Vec3::one(),
            },
            Vertex2D {
                pos: start,
                colour: Vec3::one(),
            },
        ]);
    }
}

fn to_wgpu(point: Vec2, dimensions: &Dimensions) -> Vec2 {
    let dimensions = dimensions.to_vec();

    let scaled = point / dimensions * 2.0;
    Vec2::new(scaled.x - 1.0, 1.0 - scaled.y)
}

pub fn render_buttons(
    selected_button: Res<SelectedButton>,
    mut lines_2d: ResMut<GpuBuffer<Vertex2D>>,
    dimensions: Res<Dimensions>,
) {
    if let Some(i) = selected_button.0 {
        let colour = Vec3::one();

        let offset = i + 1 + UnitButtons::UI_LINES as usize;

        lines_2d.stage(&[
            Vertex2D {
                pos: to_wgpu(
                    Vec2::new(0.0, offset as f32 * UnitButtons::LINE_HEIGHT),
                    &dimensions,
                ),
                colour,
            },
            Vertex2D {
                pos: to_wgpu(
                    Vec2::new(
                        UnitButtons::BUTTON_WIDTH,
                        offset as f32 * UnitButtons::LINE_HEIGHT,
                    ),
                    &dimensions,
                ),
                colour,
            },
        ])
    }
}

#[profiling::function]
pub fn render_health(
    query: Query<
        (
            &Position,
            Option<&Health>,
            Option<&Selected>,
            Option<&Carrying>,
            Option<&OnBoard>,
            Option<&StoredMinerals>,
            Option<&CanBeMined>,
            Option<&BuildQueue>,
        ),
        Without<Enemy>,
    >,
    people: Query<&PersonType>,
    mut glyph_brush: ResMut<GlyphBrush>,
    perspective_view: Res<PerspectiveView>,
    dimensions: Res<Dimensions>,
    total_time: Res<TotalTime>,
) {
    query.for_each(
        |(pos, health, selected, carrying, on_board, minerals, can_be_mined, build_queue)| {
            let projected =
                perspective_view.perspective_view * Vec4::new(pos.0.x, pos.0.y, pos.0.z, 1.0);

            if projected.z > 0.0 {
                let screen_space_pos = Vec2::new(projected.x, projected.y) / projected.w;

                let uv_space_pos = Vec2::new(
                    (screen_space_pos.x + 1.0) / 2.0,
                    (1.0 - screen_space_pos.y) / 2.0,
                );
                let unnormalised_pos = uv_space_pos * dimensions.to_vec();

                let selected = selected.is_some();

                let mut section =
                    glyph_brush::OwnedSection::default().with_screen_position(unnormalised_pos);

                if let Some(health) = health {
                    if selected || health.current < health.max {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!("Health: {:.2}\n", health.current))
                                .with_color([1.0; 4]),
                        );
                    }
                }

                if let Some(carrying) = carrying {
                    if selected || !carrying.0.is_empty() {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!(
                                "Carrying: {}/{}\n",
                                carrying.0.len(),
                                carrying.0.capacity()
                            ))
                            .with_color([1.0; 4]),
                        );
                    }
                }

                if let Some(on_board) = on_board {
                    if selected {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!(
                                "On Board: {}\n",
                                on_board.0.len()
                            ))
                            .with_color([1.0; 4]),
                        );

                        let mut counts = [0; PersonType::COUNT];

                        on_board.0.iter().for_each(|&entity| {
                            if let Ok(person_ty) = people.get(entity) {
                                counts[*person_ty as usize] += 1;
                            }
                        });

                        for person_ty in IntoIter::new(PersonType::ARRAY) {
                            let count = counts[person_ty as usize];

                            if count > 0 {
                                section = section.add_text(
                                    glyph_brush::OwnedText::new(format!(
                                        "  - {:?}s: {}\n",
                                        person_ty, count
                                    ))
                                    .with_color([1.0; 4]),
                                );
                            }
                        }
                    }
                }

                if let Some(minerals) = minerals {
                    if selected || minerals.stored > 0.0 {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!(
                                "Minerals: {:.2}/{:.2}\n",
                                minerals.stored, minerals.capacity
                            ))
                            .with_color([1.0; 4]),
                        );
                    }
                }

                if let Some(can_be_mined) = can_be_mined {
                    if selected || can_be_mined.minerals < can_be_mined.total {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!(
                                "Remaining Minerals: {:.2}/{:.2}\n",
                                can_be_mined.minerals, can_be_mined.total
                            ))
                            .with_color([1.0; 4]),
                        );
                    }
                }

                if let Some(build_queue) = build_queue {
                    let progress = build_queue.progress_time(total_time.0);

                    if selected || progress.is_some() {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!(
                                "Building Ships: {}\n",
                                build_queue.num_in_queue()
                            ))
                            .with_color([1.0; 4]),
                        );
                    }

                    if let Some(progress) = progress {
                        section = section.add_text(
                            glyph_brush::OwnedText::new(format!(
                                "  - Progress: {:.2}%\n",
                                progress * 100.0
                            ))
                            .with_color([1.0; 4]),
                        );
                    }
                }

                glyph_brush.queue(&section);
            }
        },
    )
}

#[profiling::function]
pub fn debug_render_tlas(
    tlas: Res<TopLevelAccelerationStructure>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
    settings: Res<Settings>,
) {
    if !settings.debug_render_tlas {
        return;
    }

    tlas.iter_bounding_boxes()
        .for_each(|(bounding_box, is_root)| {
            let colour = if is_root {
                Vec3::unit_y()
            } else {
                Vec3::unit_z()
            };

            for point in IntoIter::new(bounding_box.line_points()) {
                lines_buffer.stage(&[BackgroundVertex {
                    position: point,
                    colour,
                }])
            }
        })
}
