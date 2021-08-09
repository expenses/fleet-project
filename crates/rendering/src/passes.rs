use crate::{Pipelines, Resizables};
use components_and_resources::components::ModelId;
use components_and_resources::gpu_structs::{
    BlurSettings, CircleInstance, ColouredVertex, GodraySettings, LaserVertex, PushConstants,
    RangeInstance, Vertex2D,
};
use components_and_resources::resources;
use ultraviolet::{Vec2, Vec3, Vec4};

pub struct StarSystem {
    pub sun_dir: Vec3,
    pub background_vertices: wgpu::Buffer,
    pub num_background_vertices: u32,
    pub ambient_light: Vec3,
}

pub struct Constants {
    pub bounding_box_indices: wgpu::Buffer,
    pub circle_vertices: wgpu::Buffer,
    pub circle_line_indices: wgpu::Buffer,
    pub circle_filled_indices: wgpu::Buffer,
}

pub fn run_render_passes(
    frame: &wgpu::SwapChainFrame,
    encoder: &mut wgpu::CommandEncoder,
    resizables: &Resizables,
    pipelines: &Pipelines,
    world: &bevy_ecs::world::World,
    star_system: &StarSystem,
    tonemapper: &colstodian::tonemap::BakedLottesTonemapperParams,
    constants: &Constants,
) {
    let ship_buffer = world.get_resource::<resources::ShipBuffer>().unwrap();
    let models = world.get_resource::<resources::Models>().unwrap();
    let perspective_view = world.get_resource::<resources::PerspectiveView>().unwrap();
    let settings = world.get_resource::<resources::Settings>().unwrap();

    let laser_buffer = world
        .get_resource::<resources::GpuBuffer<LaserVertex>>()
        .unwrap();

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("main render pass"),
        color_attachments: &[
            wgpu::RenderPassColorAttachment {
                view: &resizables.hdr_framebuffer,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            },
            wgpu::RenderPassColorAttachment {
                view: &resizables.bloom_buffer,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            },
            wgpu::RenderPassColorAttachment {
                view: &resizables.godray_buffer,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            },
        ],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &resizables.depth_buffer,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(1.0),
                store: true,
            }),
            stencil_ops: None,
        }),
    });

    let (instance_buffer, num_instances, draw_indirect_buffer, draw_indirect_count) =
        ship_buffer.slice();

    render_pass.set_pipeline(&pipelines.ship);
    render_pass.set_push_constants(
        wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
        0,
        bytemuck::bytes_of(&PushConstants {
            perspective_view: perspective_view.perspective_view,
            light_dir: star_system.sun_dir,
            padding: 0,
            ambient_light: star_system.ambient_light,
        }),
    );
    render_pass.set_vertex_buffer(0, models.vertices.slice(..));
    render_pass.set_vertex_buffer(1, instance_buffer);
    render_pass.set_index_buffer(models.indices.slice(..), wgpu::IndexFormat::Uint16);
    render_pass.set_bind_group(0, &models.bind_group, &[]);

    render_pass.multi_draw_indexed_indirect(&draw_indirect_buffer, 0, draw_indirect_count);

    let (laser_buffer, num_laser_vertices) = laser_buffer.slice();

    if num_laser_vertices > 0 {
        render_pass.set_pipeline(&pipelines.lasers);
        render_pass.set_vertex_buffer(0, laser_buffer);
        render_pass.set_push_constants(
            wgpu::ShaderStage::VERTEX,
            0,
            bytemuck::bytes_of(&perspective_view.perspective_view),
        );
        render_pass.draw(0..num_laser_vertices, 0..1);
    }

    render_pass.set_pipeline(&pipelines.background);
    render_pass.set_vertex_buffer(0, star_system.background_vertices.slice(..));
    render_pass.set_push_constants(
        wgpu::ShaderStage::VERTEX,
        0,
        bytemuck::bytes_of(&perspective_view.perspective_view_without_movement),
    );
    render_pass.draw(0..star_system.num_background_vertices, 0..1);

    drop(render_pass);

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("first bloom blur render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view: &resizables.intermediate_bloom_buffer,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }],
        depth_stencil_attachment: None,
    });

    render_pass.set_pipeline(&pipelines.first_bloom_blur);
    render_pass.set_bind_group(0, &resizables.first_bloom_blur_pass, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStage::FRAGMENT,
        0,
        bytemuck::bytes_of(&BlurSettings {
            direction: 0,
            strength: 1.0,
            scale: 2.0,
        }),
    );
    render_pass.draw(0..3, 0..1);

    drop(render_pass);

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("second bloom blur render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view: &resizables.hdr_framebuffer,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
        }],
        depth_stencil_attachment: None,
    });

    render_pass.set_pipeline(&pipelines.second_bloom_blur);
    render_pass.set_bind_group(0, &resizables.second_bloom_blur_pass, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStage::FRAGMENT,
        0,
        bytemuck::bytes_of(&BlurSettings {
            direction: 1,
            strength: 1.0,
            scale: 1.0,
        }),
    );
    render_pass.draw(0..3, 0..1);

    if settings.draw_godrays {
        let uv_space_light_pos = uv_space_light_pos(&perspective_view, star_system.sun_dir);

        render_pass.set_pipeline(&pipelines.godray_blur);
        render_pass.set_bind_group(0, &resizables.godray_bind_group, &[]);
        render_pass.set_push_constants(
            wgpu::ShaderStage::FRAGMENT,
            0,
            bytemuck::bytes_of(&GodraySettings {
                density_div_num_samples: 1.0 / 100.0,
                decay: 0.98,
                weight: 0.01,
                num_samples: 100,
                uv_space_light_pos,
            }),
        );
        render_pass.draw(0..3, 0..1);
    }

    drop(render_pass);

    let circle_instances_buffer = world
        .get_resource::<resources::GpuBuffer<CircleInstance>>()
        .unwrap();

    let range_instances_buffer = world
        .get_resource::<resources::GpuBuffer<RangeInstance>>()
        .unwrap();

    let lines_2d_buffer = world
        .get_resource::<resources::GpuBuffer<Vertex2D>>()
        .unwrap();

    let line_buffer = world
        .get_resource::<resources::GpuBuffer<ColouredVertex>>()
        .unwrap();

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("tonemap and ui render pass"),
        color_attachments: &[wgpu::RenderPassColorAttachment {
            view: &frame.output.view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: true,
            },
        }],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &resizables.depth_buffer,
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            }),
            stencil_ops: None,
        }),
    });

    render_pass.set_pipeline(&pipelines.tonemapper);
    render_pass.set_bind_group(0, &resizables.hdr_pass, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStage::FRAGMENT,
        0,
        bytemuck::bytes_of(tonemapper),
    );
    render_pass.draw(0..3, 0..1);

    let (line_buffer, num_line_vertices) = line_buffer.slice();

    if num_line_vertices > 0 {
        render_pass.set_pipeline(&pipelines.lines);
        render_pass.set_vertex_buffer(0, line_buffer);
        render_pass.set_push_constants(
            wgpu::ShaderStage::VERTEX,
            0,
            bytemuck::bytes_of(&perspective_view.perspective_view),
        );
        render_pass.draw(0..num_line_vertices, 0..1);
    }

    {
        render_pass.set_pipeline(&pipelines.bounding_boxes);
        render_pass.set_push_constants(
            wgpu::ShaderStage::VERTEX,
            0,
            bytemuck::bytes_of(&perspective_view.perspective_view),
        );
        render_pass.set_index_buffer(
            constants.bounding_box_indices.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.set_vertex_buffer(0, models.bounding_boxes.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer);

        let mut offset = 0;
        let mut vertex_offset = 0;

        for i in 0..resources::Models::COUNT {
            let num_instances = num_instances[i];

            if num_instances > 0 {
                if i != ModelId::Explosion as usize {
                    render_pass.draw_indexed(0..24, vertex_offset, offset..offset + num_instances);
                }

                offset += num_instances;
            }

            vertex_offset += 8;
        }
    }

    let (circle_instances_buffer, num_circle_instances) = circle_instances_buffer.slice();

    if num_circle_instances > 0 {
        render_pass.set_pipeline(&pipelines.circle);
        render_pass.set_vertex_buffer(0, constants.circle_vertices.slice(..));
        render_pass.set_index_buffer(
            constants.circle_filled_indices.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.set_vertex_buffer(1, circle_instances_buffer);
        render_pass.draw_indexed(0..((64 - 2) * 3), 0, 0..num_circle_instances);

        render_pass.set_pipeline(&pipelines.circle_outline);
        render_pass.set_index_buffer(
            constants.circle_line_indices.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..(64 * 2), 0, 0..num_circle_instances);
    }

    let (range_instances_buffer, num_range_instances) = range_instances_buffer.slice();

    if num_range_instances > 0 {
        render_pass.set_pipeline(&pipelines.z_facing_circle_outline);
        render_pass.set_push_constants(
            wgpu::ShaderStage::VERTEX,
            0,
            bytemuck::bytes_of(&[perspective_view.perspective, perspective_view.view]),
        );
        render_pass.set_vertex_buffer(0, constants.circle_vertices.slice(..));
        render_pass.set_vertex_buffer(1, range_instances_buffer);
        render_pass.set_index_buffer(
            constants.circle_line_indices.slice(..),
            wgpu::IndexFormat::Uint16,
        );
        render_pass.draw_indexed(0..(64 * 2), 0, 0..num_range_instances);
    }

    let (lines_2d_buffer, num_lines_2d) = lines_2d_buffer.slice();

    if num_lines_2d > 0 {
        render_pass.set_pipeline(&pipelines.lines_2d);
        render_pass.set_vertex_buffer(0, lines_2d_buffer);
        render_pass.draw(0..num_lines_2d, 0..1);
    }

    drop(render_pass);

    let mut staging_belt = wgpu::util::StagingBelt::new(100);

    let dimensions = world.get_resource::<resources::Dimensions>().unwrap();
    let gpu_interface = world.get_resource::<resources::GpuInterface>().unwrap();
    let (width, height) = (dimensions.width, dimensions.height);

    let mut glyph_layout_cache =
        unsafe { world.get_resource_unchecked_mut::<resources::GlyphLayoutCache>() }.unwrap();

    glyph_layout_cache
        .glyph_brush()
        .draw_queued(
            &gpu_interface.device,
            &mut staging_belt,
            encoder,
            &frame.output.view,
            width,
            height,
        )
        .unwrap();
}

fn uv_space_light_pos(perspective_view: &resources::PerspectiveView, sun_dir: Vec3) -> Vec2 {
    let projected = perspective_view.perspective_view_without_movement
        * Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, 1.0);

    let screen_space_pos = Vec2::new(projected.x, projected.y) / projected.w;

    // wgpu correction
    Vec2::new(
        (screen_space_pos.x + 1.0) / 2.0,
        (1.0 - screen_space_pos.y) / 2.0,
    )
}
