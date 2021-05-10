use crate::gpu_structs::{BackgroundVertex, KawaseSettings, GodraySettings, PushConstants};
use crate::resources;
use crate::{Pipelines, Resizables};
use ultraviolet::{Vec2, Vec3, Vec4};

pub struct StarSystem {
    pub sun_dir: Vec3,
    pub background_vertices: wgpu::Buffer,
    pub num_background_vertices: u32,
    pub star_vertices: wgpu::Buffer,
    pub num_stars: u32,
}

pub struct Constants {
    pub bounding_box_indices: wgpu::Buffer,
}

pub fn run_render_passes(
    frame: &wgpu::SwapChainFrame,
    encoder: &mut wgpu::CommandEncoder,
    resizables: &Resizables,
    pipelines: &Pipelines,
    resources: &legion::Resources,
    star_system: &StarSystem,
    tonemapper: &colstodian::LottesTonemapper,
    constants: &Constants,
    draw_godrays: bool,
) {
    let ship_buffer = resources.get::<resources::ShipBuffer>().unwrap();
    let models = resources.get::<resources::Models>().unwrap();
    let line_buffer = resources
        .get::<resources::GpuBuffer<BackgroundVertex>>()
        .unwrap();
    let perspective_view = resources.get::<resources::PerspectiveView>().unwrap();
    let dimensions = resources.get::<resources::Dimensions>().unwrap();
    let resolution = Vec2::new(dimensions.width as f32, dimensions.height as f32);

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

    let (instance_buffer, num_instances) = ship_buffer.slice();

    render_pass.set_pipeline(&pipelines.ship);
    render_pass.set_bind_group(0, &models.carrier.bind_group, &[]);
    render_pass.set_vertex_buffer(0, models.carrier.vertices.slice(..));
    render_pass.set_vertex_buffer(1, instance_buffer);
    render_pass.set_index_buffer(models.carrier.indices.slice(..), wgpu::IndexFormat::Uint16);
    render_pass.set_push_constants(
        wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
        0,
        bytemuck::bytes_of(&PushConstants {
            perspective_view: perspective_view.perspective_view,
            light_dir: star_system.sun_dir,
        }),
    );
    render_pass.draw_indexed(0..models.carrier.num_indices, 0, 0..num_instances[0]);

    render_pass.set_pipeline(&pipelines.background);
    render_pass.set_vertex_buffer(0, star_system.background_vertices.slice(..));
    render_pass.set_push_constants(
        wgpu::ShaderStage::VERTEX,
        0,
        bytemuck::bytes_of(&perspective_view.perspective_view_without_movement),
    );
    render_pass.draw(0..star_system.num_background_vertices, 0..1);

    render_pass.set_vertex_buffer(0, star_system.star_vertices.slice(..));
    render_pass.draw(0..star_system.num_stars, 0..1);

    drop(render_pass);

    let settings = KawaseSettings { half_offset_per_pixel: Vec2::broadcast(1.5) / resolution };
    let settings_bytes = bytemuck::bytes_of(&settings);

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kawase downsample render pass 1"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &resizables.kawase_bloom_buffer_1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&pipelines.kawase_downsample);
        render_pass.set_bind_group(0, &resizables.kawase_downsample_pass_1, &[]);
        render_pass.set_push_constants(
            wgpu::ShaderStage::FRAGMENT,
            0,
            settings_bytes,
        );
        render_pass.draw(0..3, 0..1);

        drop(render_pass);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kawase downsample render pass 2"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &resizables.kawase_bloom_buffer_2,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&pipelines.kawase_downsample);
        render_pass.set_bind_group(0, &resizables.kawase_downsample_pass_2, &[]);
        render_pass.set_push_constants(
            wgpu::ShaderStage::FRAGMENT,
            0,
            settings_bytes,
        );
        render_pass.draw(0..3, 0..1);

        drop(render_pass);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("kawase upsample render pass 1"),
            color_attachments: &[wgpu::RenderPassColorAttachment {
                view: &resizables.kawase_bloom_buffer_1,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: true,
                },
            }],
            depth_stencil_attachment: None,
        });

        render_pass.set_pipeline(&pipelines.kawase_upsample);
        render_pass.set_bind_group(0, &resizables.kawase_upsample_pass_1, &[]);
        render_pass.set_push_constants(
            wgpu::ShaderStage::FRAGMENT,
            0,
            settings_bytes,
        );
        render_pass.draw(0..3, 0..1);

        drop(render_pass);
    }

    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("kawase upsample final"),
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

    render_pass.set_pipeline(&pipelines.kawase_upsample);
    render_pass.set_bind_group(0, &resizables.kawase_upsample_pass_2, &[]);
    render_pass.set_push_constants(
        wgpu::ShaderStage::FRAGMENT,
        0,
        settings_bytes,
    );
    render_pass.draw(0..3, 0..1);

    if draw_godrays {
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

    render_pass.set_pipeline(&pipelines.lines);
    render_pass.set_vertex_buffer(0, line_buffer);
    render_pass.set_push_constants(
        wgpu::ShaderStage::VERTEX,
        0,
        bytemuck::bytes_of(&perspective_view.perspective_view),
    );
    render_pass.draw(0..num_line_vertices, 0..1);

    render_pass.set_pipeline(&pipelines.bounding_boxes);
    render_pass.set_vertex_buffer(0, models.carrier.bounding_box_buffer.slice(..));
    render_pass.set_index_buffer(
        constants.bounding_box_indices.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    render_pass.set_vertex_buffer(1, instance_buffer);
    render_pass.draw_indexed(0..24, 0, 0..num_instances[0]);

    drop(render_pass);
}

fn uv_space_light_pos(perspective_view: &resources::PerspectiveView, sun_dir: Vec3) -> Vec2 {
    let projected = perspective_view.perspective_view_without_movement
        * Vec4::new(sun_dir.x, sun_dir.y, sun_dir.z, 1.0);

    let screen_space_pos = Vec2::new(projected.x, projected.y) / projected.w;

    let wgpu_corrected = Vec2::new(
        (screen_space_pos.x + 1.0) / 2.0,
        (1.0 - screen_space_pos.y) / 2.0,
    );

    wgpu_corrected
}
