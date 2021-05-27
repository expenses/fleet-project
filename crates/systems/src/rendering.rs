use super::get_scale;
use bevy_ecs::prelude::*;
use components_and_resources::components::*;
use components_and_resources::gpu_structs::{
    BackgroundVertex, CircleInstance, Instance, LaserVertex, RangeInstance, Vertex2D,
};
use components_and_resources::resources::*;
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
    )>,
    ship_under_cursor: Res<ShipUnderCursor>,
    mut ship_buffer: ResMut<ShipBuffer>,
) {
    query.for_each(
        |(entity, selected, position, rotation_matrix, model_id, scale, friendly, enemy)| {
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

            ship_buffer.stage(
                Instance {
                    translation: position.0,
                    rotation: rotation_matrix.matrix,
                    colour,
                    scale: get_scale(scale),
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
    query: Query<(&Position, &Command), With<Selected>>,
    positions: Query<&Position>,
    mut lines_buffer: ResMut<GpuBuffer<BackgroundVertex>>,
) {
    query.for_each(|(position, command)| {
        if let Command::Interact {
            target,
            ty: InteractionType::Attack,
        } = *command
        {
            if let Ok(target_pos) = positions.get(target) {
                lines_buffer.stage(&[
                    BackgroundVertex {
                        position: position.0,
                        colour: Vec3::zero(),
                    },
                    BackgroundVertex {
                        position: target_pos.0,
                        colour: Vec3::one(),
                    },
                ]);
            }
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
        let dimensions = dimensions.to_vec();

        let to_wgpu = |pos: Vec2| {
            let scaled = pos / dimensions * 2.0;
            Vec2::new(scaled.x - 1.0, 1.0 - scaled.y)
        };

        let start = to_wgpu(start);
        let end = to_wgpu(mouse_state.position);

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
