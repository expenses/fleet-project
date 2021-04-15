use ultraviolet::{Mat3, Mat4, Vec2, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::event::*;
use winit::event_loop::*;

mod background;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::Window::new(&event_loop)?;

    let surface = unsafe { instance.create_surface(&window) };

    let adapter = pollster::block_on(instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
            }))
            .ok_or_else(|| anyhow::anyhow!(
                "'request_adapter' failed. If you get this on linux, try installing the vulkan drivers for your gpu. \
                You can check that they're working properly by running `vulkaninfo` or `vkcube`."
            ))?;

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("device"),
            features: wgpu::Features::PUSH_CONSTANTS | wgpu::Features::DEPTH_CLAMPING,
            limits: wgpu::Limits {
                max_push_constant_size: std::mem::size_of::<PushConstants>() as u32,
                ..Default::default()
            },
        },
        None,
    ))?;

    let display_format = adapter.get_swap_chain_preferred_format(&surface).unwrap();
    let window_size = window.inner_size();
    let width = window_size.width;
    let height = window_size.height;

    let resources = Resources::new(&device);
    let pipelines = Pipelines::new(&device, &resources, display_format);

    let carrier = load_ship_model(
        include_bytes!("../models/carrier.glb"),
        &device,
        &queue,
        &resources,
    )?;

    let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        usage: wgpu::BufferUsage::VERTEX,
        contents: bytemuck::bytes_of(&Instance {
            rotation: Mat3::identity(),
            translation: Vec3::zero(),
        }),
    });

    let mut orbit = Orbit {
        latitude: 0.0,
        longitude: 1.0,
        distance: 10.0,
    };

    let perspective = ultraviolet::projection::perspective_wgpu_dx(
        59.0_f32.to_radians(),
        width as f32 / height as f32,
        0.1,
        100.0,
    );

    let mut perspective_view = PerspectiveView::new(perspective, orbit.as_vector(), Vec3::zero());
    let mut mouse_down = false;
    let mut previous_cursor_position = Vec2::zero();

    let mut resizables =
        Resizables::new(width, height, display_format, &device, &surface, &resources);

    let mut rng = rand::thread_rng();
    let background = background::make_background(&mut rng);
    let num_background_vertices = background.len() as u32;
    let background_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("background vertices"),
        contents: bytemuck::cast_slice(&background),
        usage: wgpu::BufferUsage::VERTEX,
    });

    let mut sun_dir = background::uniform_sphere_distribution(&mut rng);
    sun_dir.y = sun_dir.y.abs();

    let stars = background::create_stars(&mut rng)
        .chain(background::star_points(sun_dir, 250.0, Vec3::broadcast(2.0)))
        .collect::<Vec<_>>();
    let num_stars = stars.len() as u32;
    let star_vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("star vertices"),
        contents: bytemuck::cast_slice(&stars),
        usage: wgpu::BufferUsage::VERTEX,
    });

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { ref event, .. } => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::Resized(size) => {
                let width = size.width as u32;
                let height = size.height as u32;

                resizables =
                    Resizables::new(width, height, display_format, &device, &surface, &resources);

                perspective_view.set_perspective(ultraviolet::projection::perspective_wgpu_dx(
                    59.0_f32.to_radians(),
                    width as f32 / height as f32,
                    0.1,
                    100.0,
                ))
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                mouse_down = *state == ElementState::Pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical::<f32>(window.scale_factor());
                let position = Vec2::new(position.x, position.y);

                if mouse_down {
                    let delta = position - previous_cursor_position;
                    orbit.rotate(delta);
                    perspective_view.set_view(orbit.as_vector(), Vec3::zero());
                }

                previous_cursor_position = position;
            }
            _ => {}
        },
        Event::MainEventsCleared => window.request_redraw(),
        Event::RedrawRequested(_) => {
            if let Ok(frame) = resizables.swapchain.get_current_frame() {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render encoder"),
                });

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main render pass"),
                    color_attachments: &[
                        wgpu::RenderPassColorAttachment {
                            view: &frame.output.view,
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

                render_pass.set_pipeline(&pipelines.ship);
                render_pass.set_bind_group(0, &carrier.bind_group, &[]);
                render_pass.set_vertex_buffer(0, carrier.vertices.slice(..));
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
                render_pass.set_index_buffer(carrier.indices.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.set_push_constants(
                    wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&PushConstants {
                        perspective_view: perspective_view.perspective_view,
                        light_dir: sun_dir,
                    }),
                );
                render_pass.draw_indexed(0..carrier.num_indices, 0, 0..1);

                render_pass.set_pipeline(&pipelines.background);
                render_pass.set_vertex_buffer(0, background_vertices.slice(..));
                render_pass.set_push_constants(
                    wgpu::ShaderStage::VERTEX,
                    0,
                    bytemuck::bytes_of(&perspective_view.perspective_view_without_movement),
                );
                render_pass.draw(0..num_background_vertices, 0..1);

                render_pass.set_vertex_buffer(0, star_vertices.slice(..));
                render_pass.draw(0..num_stars, 0..1);

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

                render_pass.set_pipeline(&pipelines.blur);
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
                        view: &frame.output.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                });

                render_pass.set_pipeline(&pipelines.blur);
                render_pass.set_bind_group(0, &resizables.second_bloom_blur_pass, &[]);
                render_pass.set_push_constants(
                    wgpu::ShaderStage::FRAGMENT,
                    0,
                    bytemuck::bytes_of(&BlurSettings {
                        direction: 1,
                        strength: 1.0,
                        scale: 2.0,
                    }),
                );
                render_pass.draw(0..3, 0..1);

                let uv_space_light_pos = {
                    let projected = perspective_view.perspective_view_without_movement
                        * Vec4::new(sun_dir.x * 100.0, sun_dir.y * 100.0, sun_dir.z * 100.0, 1.0);

                    let screen_space_pos = Vec2::new(projected.x, projected.y) / projected.w;

                    let wgpu_corrected = Vec2::new(
                        (screen_space_pos.x + 1.0) / 2.0,
                        (1.0 - screen_space_pos.y) / 2.0,
                    );

                    wgpu_corrected
                };

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

                drop(render_pass);

                queue.submit(Some(encoder.finish()));
            }
        }
        _ => {}
    })
}

struct Resizables {
    swapchain: wgpu::SwapChain,
    depth_buffer: wgpu::TextureView,
    bloom_buffer: wgpu::TextureView,
    intermediate_bloom_buffer: wgpu::TextureView,
    first_bloom_blur_pass: wgpu::BindGroup,
    second_bloom_blur_pass: wgpu::BindGroup,
    godray_buffer: wgpu::TextureView,
    godray_bind_group: wgpu::BindGroup,
}

impl Resizables {
    fn new(
        width: u32,
        height: u32,
        display_format: wgpu::TextureFormat,
        device: &wgpu::Device,
        surface: &wgpu::Surface,
        resources: &Resources,
    ) -> Self {
        let bloom_buffer = create_texture(
            device,
            "bloom buffer",
            width,
            height,
            display_format,
            wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        );

        let intermediate_bloom_buffer = create_texture(
            device,
            "intermediate bloom buffer",
            width,
            height,
            display_format,
            wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        );

        let godray_buffer = create_texture(
            &device,
            "godray buffer",
            width,
            height,
            display_format,
            wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        );

        Self {
            swapchain: device.create_swap_chain(
                surface,
                &wgpu::SwapChainDescriptor {
                    width,
                    height,
                    usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
                    format: display_format,
                    present_mode: wgpu::PresentMode::Fifo,
                },
            ),
            depth_buffer: create_texture(
                &device,
                "depth buffer",
                width,
                height,
                DEPTH_FORMAT,
                wgpu::TextureUsage::RENDER_ATTACHMENT,
            ),
            first_bloom_blur_pass: make_effect_bind_group(
                &device,
                &resources,
                &bloom_buffer,
                "first bloom blur pass bind group",
            ),
            bloom_buffer,
            second_bloom_blur_pass: make_effect_bind_group(
                &device,
                &resources,
                &intermediate_bloom_buffer,
                "second bloom blur pass bind group",
            ),
            intermediate_bloom_buffer,
            godray_bind_group: make_effect_bind_group(
                &device,
                &resources,
                &godray_buffer,
                "godray blur bind group",
            ),
            godray_buffer,
        }
    }
}

fn make_effect_bind_group(
    device: &wgpu::Device,
    resources: &Resources,
    source: &wgpu::TextureView,
    label: &str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &resources.effect_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&resources.linear_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(source),
            },
        ],
    })
}

struct PerspectiveView {
    perspective: Mat4,
    view: Mat4,
    view_without_movement: Mat4,
    perspective_view: Mat4,
    perspective_view_without_movement: Mat4,
}

impl PerspectiveView {
    fn new(perspective: Mat4, eye: Vec3, center: Vec3) -> Self {
        let view = Mat4::look_at(eye + center, center, Vec3::unit_y());
        let view_without_movement = Mat4::look_at(Vec3::zero(), -eye, Vec3::unit_y());

        Self {
            view,
            view_without_movement,
            perspective,
            perspective_view: perspective * view,
            perspective_view_without_movement: perspective * view_without_movement,
        }
    }

    fn set_perspective(&mut self, perspective: Mat4) {
        self.perspective = perspective;
        self.perspective_view = self.perspective * self.view;
        self.perspective_view_without_movement = self.perspective * self.view_without_movement;
    }

    fn set_view(&mut self, eye: Vec3, center: Vec3) {
        self.view = Mat4::look_at(eye + center, center, Vec3::unit_y());
        self.perspective_view = self.perspective * self.view;
        self.view_without_movement = Mat4::look_at(Vec3::zero(), -eye, Vec3::unit_y());
        self.perspective_view_without_movement = self.perspective * self.view_without_movement;
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PushConstants {
    perspective_view: Mat4,
    light_dir: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    rotation: Mat3,
    translation: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ModelVertex {
    position: Vec3,
    normal: Vec3,
    uv: Vec2,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BackgroundVertex {
    position: Vec3,
    colour: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurSettings {
    scale: f32,
    strength: f32,
    direction: i32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GodraySettings {
    density_div_num_samples: f32,
    decay: f32,
    weight: f32,
    num_samples: u32,
    uv_space_light_pos: Vec2,
}

struct Resources {
    ship_bgl: wgpu::BindGroupLayout,
    effect_bgl: wgpu::BindGroupLayout,
    nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
}

impl Resources {
    fn new(device: &wgpu::Device) -> Self {
        let texture = |binding, shader_stage| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let sampler = |binding, shader_stage| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Sampler {
                filtering: false,
                comparison: false,
            },
            count: None,
        };

        Self {
            ship_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ship bind group layout"),
                entries: &[
                    sampler(0, wgpu::ShaderStage::FRAGMENT),
                    texture(1, wgpu::ShaderStage::FRAGMENT),
                    texture(2, wgpu::ShaderStage::FRAGMENT),
                ],
            }),
            effect_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("effect bind group layout"),
                entries: &[
                    sampler(0, wgpu::ShaderStage::FRAGMENT),
                    texture(1, wgpu::ShaderStage::FRAGMENT),
                ],
            }),
            nearest_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("nearest sampler"),
                ..Default::default()
            }),
            linear_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("linear sampler"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }
}

struct Pipelines {
    ship: wgpu::RenderPipeline,
    background: wgpu::RenderPipeline,
    blur: wgpu::RenderPipeline,
    godray_blur: wgpu::RenderPipeline,
}

impl Pipelines {
    fn new(
        device: &wgpu::Device,
        resources: &Resources,
        display_format: wgpu::TextureFormat,
    ) -> Self {
        let ship_bgl_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ship bgl pipeline layout"),
                bind_group_layouts: &[&resources.ship_bgl],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    range: 0..std::mem::size_of::<PushConstants>() as u32,
                }],
            });

        let model_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModelVertex>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
        };

        let instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::InputStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![3 => Float32x3, 4 => Float32x3, 5 => Float32x3, 6 => Float32x3],
        };

        let depth_write = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        let depth_read = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };

        let backface_culling = wgpu::PrimitiveState {
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        };

        let clamp_depth = wgpu::PrimitiveState {
            clamp_depth: true,
            ..Default::default()
        };

        let vs_fullscreen_tri = device.create_shader_module(&wgpu::include_spirv!(
            "../shaders/compiled/fullscreen_tri.vert.spv"
        ));

        let additive_colour_state = wgpu::ColorTargetState {
            format: display_format,
            write_mask: wgpu::ColorWrite::ALL,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    operation: wgpu::BlendOperation::Add,
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
        };

        let ignore_colour_state = wgpu::ColorTargetState {
            format: display_format,
            write_mask: wgpu::ColorWrite::empty(),
            blend: None,
        };

        Self {
            ship: {
                let vs_ship = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/ship.vert.spv"
                ));

                let fs_ship = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/ship.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("ship pipeline"),
                    layout: Some(&ship_bgl_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_ship,
                        entry_point: "main",
                        buffers: &[
                            model_vertex_buffer_layout.clone(),
                            instance_buffer_layout.clone(),
                        ],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_ship,
                        entry_point: "main",
                        targets: &[
                            display_format.into(),
                            display_format.into(),
                            ignore_colour_state,
                        ],
                    }),
                    primitive: backface_culling.clone(),
                    depth_stencil: Some(depth_write.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            background: {
                let background_vertex_buffer_layout = wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BackgroundVertex>() as u64,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                };

                let background_pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("background pipeline layout"),
                        bind_group_layouts: &[],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStage::VERTEX,
                            range: 0..std::mem::size_of::<Mat4>() as u32,
                        }],
                    });

                let vs_background = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/background.vert.spv"
                ));

                let fs_background = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/background.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("background pipeline"),
                    layout: Some(&background_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_background,
                        entry_point: "main",
                        buffers: &[background_vertex_buffer_layout],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_background,
                        entry_point: "main",
                        targets: &[
                            display_format.into(),
                            display_format.into(),
                            display_format.into(),
                        ],
                    }),
                    primitive: clamp_depth.clone(),
                    depth_stencil: Some(depth_read.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            blur: {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("bloom blur pipeline layout"),
                        bind_group_layouts: &[&resources.effect_bgl],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStage::FRAGMENT,
                            range: 0..std::mem::size_of::<BlurSettings>() as u32,
                        }],
                    });

                let fs_blur = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/blur.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("bloom blur pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_fullscreen_tri,
                        entry_point: "main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_blur,
                        entry_point: "main",
                        targets: &[additive_colour_state.clone()],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            godray_blur: {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("godray blur pipeline layout"),
                        bind_group_layouts: &[&resources.effect_bgl],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStage::FRAGMENT,
                            range: 0..std::mem::size_of::<GodraySettings>() as u32,
                        }],
                    });

                let fs_godray_blur = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
                    label: Some("fs godray blur"),
                    source: wgpu::util::make_spirv(include_bytes!(
                        "../shaders/compiled/godray_blur.frag.spv"
                    )),
                    flags: wgpu::ShaderFlags::empty(),
                });

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("godray blur pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_fullscreen_tri,
                        entry_point: "main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_godray_blur,
                        entry_point: "main",
                        targets: &[additive_colour_state],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                })
            },
        }
    }
}

fn create_texture(
    device: &wgpu::Device,
    label: &str,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsage,
) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

struct Model {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    num_indices: u32,
    bind_group: wgpu::BindGroup,
}

fn load_ship_model(
    bytes: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    resources: &Resources,
) -> anyhow::Result<Model> {
    let gltf = gltf::Gltf::from_slice(bytes)?;

    let buffer_blob = gltf.blob.as_ref().unwrap();

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| {
                assert_eq!(buffer.index(), 0);
                Some(buffer_blob)
            });

            let num_vertices = vertices.len() as u16;

            let read_indices = match reader.read_indices().unwrap() {
                gltf::mesh::util::ReadIndices::U16(indices) => indices,
                gltf::mesh::util::ReadIndices::U32(_) => {
                    return Err(anyhow::anyhow!("U32 indices not supported"))
                }
                _ => unreachable!(),
            };

            indices.extend(read_indices.map(|index| index + num_vertices));

            let positions = reader.read_positions().unwrap();
            let normals = reader.read_normals().unwrap();
            let uvs = reader.read_tex_coords(0).unwrap().into_f32();

            positions
                .zip(normals)
                .zip(uvs)
                .for_each(|((position, normal), uv)| {
                    vertices.push(ModelVertex {
                        position: position.into(),
                        normal: normal.into(),
                        uv: uv.into(),
                    });
                })
        }
    }

    let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        usage: wgpu::BufferUsage::VERTEX,
        contents: bytemuck::cast_slice(&vertices),
    });

    let num_indices = indices.len() as u32;

    let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        usage: wgpu::BufferUsage::INDEX,
        contents: bytemuck::cast_slice(&indices),
    });

    let material = gltf.materials().next().unwrap();

    let diffuse_texture = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .unwrap()
        .texture();
    let diffuse_texture = load_image(&diffuse_texture.source(), buffer_blob, device, queue)?;
    let emissive_texture = material.emissive_texture().unwrap().texture();
    let emissive_texture = load_image(&emissive_texture.source(), buffer_blob, device, queue)?;

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &resources.ship_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&resources.nearest_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&diffuse_texture),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&emissive_texture),
            },
        ],
    });

    Ok(Model {
        vertices,
        indices,
        num_indices,
        bind_group,
    })
}

fn load_image(
    image: &gltf::Image,
    buffer_blob: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<wgpu::TextureView> {
    let image_view = match image.source() {
        gltf::image::Source::View { view, .. } => view,
        _ => panic!(),
    };

    let image_start = image_view.offset();
    let image_end = image_start + image_view.length();
    let image_bytes = &buffer_blob[image_start..image_end];

    let image = image::load_from_memory_with_format(image_bytes, image::ImageFormat::Png)?;

    let image = match image {
        image::DynamicImage::ImageRgba8(image) => image,
        _ => panic!(),
    };

    Ok(device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: image.width(),
                    height: image.height(),
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::SAMPLED,
            },
            &*image,
        )
        .create_view(&wgpu::TextureViewDescriptor::default()))
}

pub struct Orbit {
    pub longitude: f32,
    pub latitude: f32,
    distance: f32,
}

impl Orbit {
    pub fn rotate(&mut self, delta: Vec2) {
        use std::f32::consts::PI;
        let speed = 0.15;
        self.latitude -= delta.x.to_radians() * speed;
        self.longitude = (self.longitude - delta.y.to_radians() * speed)
            .max(std::f32::EPSILON)
            .min(PI - std::f32::EPSILON);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance + delta * 0.5).max(1.0).min(10.0);
    }

    fn as_vector(&self) -> Vec3 {
        let y = self.longitude.cos();
        let horizontal_amount = self.longitude.sin();
        let x = horizontal_amount * self.latitude.sin();
        let z = horizontal_amount * self.latitude.cos();
        Vec3::new(x, y, z) * self.distance
    }
}
