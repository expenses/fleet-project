use rand::Rng;
use ultraviolet::{Mat4, Rotor3, Vec2, Vec3};
use wgpu::util::DeviceExt;
use winit::event::*;
use winit::event_loop::*;

mod background;
mod components;
mod gpu_structs;
mod rendering;
mod resources;
mod steering;
mod systems;

use gpu_structs::*;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const HDR_FRAMEBUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const EFFECT_BUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

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

    let resources = Resources::new(&device);
    let pipelines = Pipelines::new(&device, &resources, display_format);

    let draw_godrays = false;

    let tonemapper = colstodian::LottesTonemapper::new(colstodian::LottesTonemaperParams {
        gray_point_in: 0.15,
        crosstalk: 10.0,
        ..Default::default()
    });

    let dimensions = resources::Dimensions {
        width: window_size.width,
        height: window_size.height,
    };

    let mut resizables = Resizables::new(
        dimensions.width,
        dimensions.height,
        display_format,
        &device,
        &surface,
        &resources,
    );

    let mut rng = rand::thread_rng();
    let background = background::make_background(&mut rng);

    let mut sun_dir = background::uniform_sphere_distribution(&mut rng);
    sun_dir.y = sun_dir.y.abs();

    let stars = background::create_stars(&mut rng)
        .chain(background::star_points(
            sun_dir,
            250.0,
            Vec3::broadcast(2.0) * Vec3::new(1.0, 0.8, 1.0 / 3.0),
        ))
        .collect::<Vec<_>>();

    let star_system = rendering::StarSystem {
        sun_dir,
        num_background_vertices: background.len() as u32,
        background_vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("background vertices"),
            contents: bytemuck::cast_slice(&background),
            usage: wgpu::BufferUsage::VERTEX,
        }),
        num_stars: stars.len() as u32,
        star_vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("star vertices"),
            contents: bytemuck::cast_slice(&stars),
            usage: wgpu::BufferUsage::VERTEX,
        }),
    };

    let bounding_box_indices_for_model_id = |id: u16| {
        let mut bounding_box_indices: [u16; 24] = [
            0, 1, 2, 3, 4, 5, 6, 7, 0, 2, 1, 3, 4, 6, 5, 7, 0, 4, 1, 5, 2, 6, 3, 7,
        ];
        let offset = id * 24;
        for index in &mut bounding_box_indices {
            *index += offset;
        }
        bounding_box_indices
    };

    let constants = rendering::Constants {
        bounding_box_indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bounding box vertices"),
            contents: bytemuck::cast_slice(&bounding_box_indices_for_model_id(0)),
            usage: wgpu::BufferUsage::INDEX,
        }),
        circle_vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("circle vertices"),
            contents: bytemuck::cast_slice(&circle_vertices::<64>()),
            usage: wgpu::BufferUsage::VERTEX,
        }),
        circle_line_indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("circle line indices"),
            contents: bytemuck::cast_slice(&circle_line_indices::<64, { 64 * 2 }>()),
            usage: wgpu::BufferUsage::INDEX,
        }),
        circle_filled_indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("circle filled indices"),
            contents: bytemuck::cast_slice(&circle_filled_indices::<64, { (64 - 2) * 3 }>()),
            usage: wgpu::BufferUsage::INDEX,
        }),
    };

    // ecs
    let mut world = legion::world::World::default();

    /*for _ in 0..100 {
        let position = Vec3::new(
            rng.gen_range(-40.0..40.0),
            rng.gen_range(-5.0..=1.0),
            rng.gen_range(-40.0..40.0),
        );
        let rotation = Rotor3::from_rotation_xz(rng.gen_range(0.0..=360.0_f32).to_radians());

        let (model, max_speed) = if rng.gen_range(0.0..1.0) > 0.5 {
            (components::ModelId::Fighter, components::MaxSpeed(10.0))
        } else {
            (components::ModelId::Carrier, components::MaxSpeed(1.0))
        };

        if rng.gen() {
            world.push((
                components::Position(position),
                components::Rotation(rotation),
                components::RotationMatrix::default(),
                model,
                max_speed,
                components::WorldSpaceBoundingBox::default(),
                components::FollowsCommands,
                components::Friendly,
                components::Velocity(Vec3::zero()),
            ));
        } else {
            world.push((
                components::Position(position),
                components::Rotation(rotation),
                components::RotationMatrix::default(),
                model,
                max_speed,
                components::WorldSpaceBoundingBox::default(),
                components::Enemy,
                components::Velocity(Vec3::zero()),
            ));
        }
    }*/

    for _ in 0..1000 {
        let side = rng.gen_range(0.0..1.0) > 1.0 / 3.0;

        let position = Vec3::new(
            rng.gen_range(-50.0..50.0) + side as u8 as f32 * 150.0,
            rng.gen_range(-50.0..50.0),
            rng.gen_range(-50.0..50.0),
        );

        let (model, max_speed) = if true {
            (components::ModelId::Fighter, components::MaxSpeed(10.0))
        } else {
            (components::ModelId::Carrier, components::MaxSpeed(1.0))
        };

        if side {
            world.push((
                components::Position(position),
                components::Rotation(Default::default()),
                components::RotationMatrix::default(),
                model,
                max_speed,
                components::WorldSpaceBoundingBox::default(),
                components::FollowsCommands,
                components::Friendly,
                components::Velocity(Vec3::zero()),
                components::RayCooldown(rng.gen_range(0.0..1.0)),
            ));
        } else {
            world.push((
                components::Position(position),
                components::Rotation(Default::default()),
                components::RotationMatrix::default(),
                model,
                max_speed,
                components::WorldSpaceBoundingBox::default(),
                components::FollowsCommands,
                components::Enemy,
                components::Velocity(Vec3::zero()),
                components::RayCooldown(rng.gen_range(0.0..1.0)),
            ));
        }
    }

    for _ in 0..10 {
        let position = Vec3::new(
            rng.gen_range(-400.0..400.0),
            rng.gen_range(-50.0..=10.0),
            rng.gen_range(-400.0..400.0),
        );
        let facing = background::uniform_sphere_distribution(&mut rng);
        let rotation = Rotor3::from_rotation_between(Vec3::unit_y(), facing);

        world.push((
            components::Position(position),
            components::Rotation(rotation),
            components::RotationMatrix::default(),
            components::ModelId::Asteroid,
            components::WorldSpaceBoundingBox::default(),
            components::Spin::new(background::uniform_sphere_distribution(&mut rng)),
            components::Scale(rng.gen_range(1.0..5.0)),
        ));
    }

    let mut lr = legion::Resources::default();
    lr.insert(resources::ShipBuffer::new(&device));
    lr.insert(resources::GpuBuffer::<BackgroundVertex>::new(
        &device,
        "lines",
        wgpu::BufferUsage::VERTEX,
    ));
    lr.insert(resources::GpuBuffer::<CircleInstance>::new(
        &device,
        "circle instances",
        wgpu::BufferUsage::VERTEX,
    ));
    lr.insert(resources::Models([
        load_ship_model(
            include_bytes!("../models/carrier.glb"),
            &device,
            &queue,
            &resources,
        )?,
        load_ship_model(
            include_bytes!("../models/fighter.glb"),
            &device,
            &queue,
            &resources,
        )?,
        load_ship_model(
            include_bytes!("../models/explosion.glb"),
            &device,
            &queue,
            &resources,
        )?,
        load_ship_model(
            include_bytes!("../models/asteroid.glb"),
            &device,
            &queue,
            &resources,
        )?,
    ]));
    lr.insert(resources::GpuInterface { device, queue });
    lr.insert(resources::MouseState::default());
    lr.insert(resources::Ray::default());
    lr.insert(resources::ShipUnderCursor::default());
    let orbit = resources::Orbit::new();
    lr.insert(resources::PerspectiveView::new(
        ultraviolet::projection::perspective_infinite_z_wgpu_dx(
            59.0_f32.to_radians(),
            dimensions.width as f32 / dimensions.height as f32,
            0.1,
        ),
        orbit.as_vector(),
        Vec3::zero(),
    ));
    lr.insert(orbit);
    lr.insert(dimensions);
    lr.insert(resources::KeyboardState::default());
    lr.insert(resources::Camera::default());
    lr.insert(resources::DeltaTime(1.0 / 60.0));
    lr.insert(resources::TotalTime(0.0));
    lr.insert(resources::RayPlanePoint::default());
    lr.insert(resources::AverageSelectedPosition::default());
    lr.insert(resources::MouseMode::Normal);
    lr.insert(resources::Paused(false));

    let mut schedule = legion::Schedule::builder()
        // No dependencies.
        .add_system(systems::move_ships_system())
        .add_system(systems::spin_system())
        .add_system(systems::kill_temporary_system())
        .add_system(systems::expand_explosions_system())
        .add_system(systems::spawn_projectiles_system())
        .add_system(systems::update_projectiles_system())
        .add_system(systems::move_camera_system())
        .add_system(systems::set_camera_following_system())
        .add_system(systems::handle_keys_system())
        .add_system(systems::apply_staging_velocity_system())
        .add_system(systems::apply_velocity_system())
        .add_system(systems::spawn_projectile_from_ships_system())
        // Need to update what the camera is following.
        .flush()
        // Buffer clears
        .add_system(systems::clear_ship_buffer_system())
        .add_system(systems::clear_buffer_system::<BackgroundVertex>())
        .add_system(systems::clear_buffer_system::<CircleInstance>())
        // Dependent on ship positions (`move_ships_system`).
        .add_system(systems::calculate_average_selected_position_system())
        //  Dependent on average ship position (`calculate_average_selected_position_system`).
        .add_system(systems::handle_right_clicks_system())
        // Flush the command buffer adding `MovingTo`s to ships.
        .flush()
        // Dependent on `handle_right_clicks_system`.
        .add_system(systems::set_rotation_from_moving_to_system())
        .add_system(systems::move_ships_system())
        // Dependent on updated rotations.
        .add_system(systems::update_ship_rotation_matrix_system())
        // Dependent on updated rotation matrices.
        .add_system(systems::set_world_space_bounding_box_system())
        // Dependent on model movement.
        .add_system(systems::move_camera_around_following_system())
        .add_system(systems::choose_enemy_target_system())
        .flush()
        .add_system(systems::run_steering_system())
        .add_system(systems::debug_draw_targets_system())
        // Dependent on model movement and updated matrices
        .add_system(systems::collide_projectiles_system())
        // Dependent on camera movement.
        .add_system(systems::update_ray_system())
        // Dependent on an updated ray
        .add_system(systems::update_ray_plane_point_system())
        // Dependent on an updated ray, positions and matrices.
        .add_system(systems::find_ship_under_cursor_system())
        // .add_system(systems::debug_find_ship_under_cursor_system())
        // Dependent on `find_ship_under_cursor_system`.
        .add_system(systems::handle_left_click_system())
        // Staging
        .add_system(systems::render_projectiles_system())
        .add_system(systems::render_movement_circle_system())
        .add_system(systems::upload_instances_system()) // rm
        // Buffer uploads
        .add_system(systems::upload_ship_buffer_system())
        .add_system(systems::upload_buffer_system::<BackgroundVertex>())
        .add_system(systems::upload_buffer_system::<CircleInstance>())
        // Cleanup
        .add_system(systems::update_mouse_state_system())
        .add_system(systems::update_keyboard_state_system())
        .add_system(systems::increase_total_time_system())
        .build();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { ref event, .. } => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::Resized(size) => {
                let mut dimensions = lr.get_mut::<resources::Dimensions>().unwrap();
                let mut perspective_view = lr.get_mut::<resources::PerspectiveView>().unwrap();
                let gpu_interface = lr.get::<resources::GpuInterface>().unwrap();

                dimensions.width = size.width as u32;
                dimensions.height = size.height as u32;

                resizables = Resizables::new(
                    dimensions.width,
                    dimensions.height,
                    display_format,
                    &gpu_interface.device,
                    &surface,
                    &resources,
                );

                perspective_view.set_perspective(
                    ultraviolet::projection::perspective_infinite_z_wgpu_dx(
                        59.0_f32.to_radians(),
                        dimensions.width as f32 / dimensions.height as f32,
                        0.1,
                    ),
                )
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(key),
                        ..
                    },
                ..
            } => {
                let pressed = *state == ElementState::Pressed;
                let mut keyboard_state = lr.get_mut::<resources::KeyboardState>().unwrap();
                keyboard_state.handle(*key, pressed);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mut mouse_state = lr.get_mut::<resources::MouseState>().unwrap();

                let pressed = *state == ElementState::Pressed;
                let position = mouse_state.position;

                match button {
                    MouseButton::Left => mouse_state.left_state.handle(position, pressed),
                    MouseButton::Right => mouse_state.right_state.handle(position, pressed),
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let mut mouse_state = lr.get_mut::<resources::MouseState>().unwrap();
                let keyboard_state = lr.get::<resources::KeyboardState>().unwrap();
                let mut mouse_mode = lr.get_mut::<resources::MouseMode>().unwrap();

                let position = Vec2::new(position.x as f32, position.y as f32);
                let delta = position - mouse_state.position;

                if mouse_state.right_state.is_being_dragged().is_some() {
                    let mut orbit = lr.get_mut::<resources::Orbit>().unwrap();
                    orbit.rotate(delta);
                } else if keyboard_state.shift {
                    if let resources::MouseMode::Movement { plane_y } = &mut *mouse_mode {
                        *plane_y -= delta.y / 10.0;
                    }
                }

                mouse_state.position = position;
            }
            _ => {}
        },
        Event::MainEventsCleared => {
            schedule.execute(&mut world, &mut lr);

            window.request_redraw();
        }
        Event::RedrawRequested(_) => {
            if let Ok(frame) = resizables.swapchain.get_current_frame() {
                let gpu_interface = lr.get::<resources::GpuInterface>().unwrap();

                let mut encoder =
                    gpu_interface
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render encoder"),
                        });

                rendering::run_render_passes(
                    &frame,
                    &mut encoder,
                    &resizables,
                    &pipelines,
                    &lr,
                    &star_system,
                    &tonemapper,
                    &constants,
                    draw_godrays,
                );

                gpu_interface.queue.submit(Some(encoder.finish()));
            }
        }
        _ => {}
    })
}

pub struct Resizables {
    swapchain: wgpu::SwapChain,
    hdr_framebuffer: wgpu::TextureView,
    depth_buffer: wgpu::TextureView,
    bloom_buffer: wgpu::TextureView,
    intermediate_bloom_buffer: wgpu::TextureView,
    hdr_pass: wgpu::BindGroup,
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
            EFFECT_BUFFER_FORMAT,
            wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        );

        let intermediate_bloom_buffer = create_texture(
            device,
            "intermediate bloom buffer",
            width / 2,
            height / 2,
            EFFECT_BUFFER_FORMAT,
            wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        );

        let godray_buffer = create_texture(
            &device,
            "godray buffer",
            width,
            height,
            EFFECT_BUFFER_FORMAT,
            wgpu::TextureUsage::RENDER_ATTACHMENT | wgpu::TextureUsage::SAMPLED,
        );

        let hdr_framebuffer = create_texture(
            &device,
            "hdr framebuffer",
            width,
            height,
            HDR_FRAMEBUFFER_FORMAT,
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
            hdr_pass: make_effect_bind_group(&device, &resources, &hdr_framebuffer, "hdr pass"),
            hdr_framebuffer,
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

        let sampler = |binding, shader_stage, filtering| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: shader_stage,
            ty: wgpu::BindingType::Sampler {
                filtering,
                comparison: false,
            },
            count: None,
        };

        Self {
            ship_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("ship bind group layout"),
                entries: &[
                    sampler(0, wgpu::ShaderStage::FRAGMENT, false),
                    texture(1, wgpu::ShaderStage::FRAGMENT),
                    texture(2, wgpu::ShaderStage::FRAGMENT),
                ],
            }),
            effect_bgl: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("effect bind group layout"),
                entries: &[
                    sampler(0, wgpu::ShaderStage::FRAGMENT, true),
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

pub struct Pipelines {
    ship: wgpu::RenderPipeline,
    background: wgpu::RenderPipeline,
    first_bloom_blur: wgpu::RenderPipeline,
    second_bloom_blur: wgpu::RenderPipeline,
    godray_blur: wgpu::RenderPipeline,
    lines: wgpu::RenderPipeline,
    bounding_boxes: wgpu::RenderPipeline,
    tonemapper: wgpu::RenderPipeline,
    circle: wgpu::RenderPipeline,
    circle_outline: wgpu::RenderPipeline,
}

impl Pipelines {
    // We use helper structs and clone them around.
    // It would be a pain to remove the clone from the last use of the struct.
    #[allow(clippy::redundant_clone)]
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
            attributes: &wgpu::vertex_attr_array![3 => Float32x3, 4 => Float32x3, 5 => Float32x3, 6 => Float32x3, 7 => Float32x3, 8 => Float32],
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

        let depth_ignore = wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
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

        let fullscreen_tri_vertex = wgpu::VertexState {
            module: &vs_fullscreen_tri,
            entry_point: "main",
            buffers: &[],
        };

        let vs_flat_colour = device.create_shader_module(&wgpu::include_spirv!(
            "../shaders/compiled/flat_colour.vert.spv"
        ));

        let additive_colour_state = |target| wgpu::ColorTargetState {
            format: target,
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

        let ignore_colour_state = |format| wgpu::ColorTargetState {
            format,
            write_mask: wgpu::ColorWrite::empty(),
            blend: None,
        };

        let perspective_view_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("perspective view pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::VERTEX,
                    range: 0..std::mem::size_of::<Mat4>() as u32,
                }],
            });

        let background_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BackgroundVertex>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
        };

        let fs_flat_colour = device.create_shader_module(&wgpu::include_spirv!(
            "../shaders/compiled/flat_colour.frag.spv"
        ));

        let bloom_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("bloom blur pipeline layout"),
                bind_group_layouts: &[&resources.effect_bgl],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::FRAGMENT,
                    range: 0..std::mem::size_of::<BlurSettings>() as u32,
                }],
            });

        let fs_blur =
            device.create_shader_module(&wgpu::include_spirv!("../shaders/compiled/blur.frag.spv"));

        let vec3_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vec3>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3],
        };

        let vec2_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vec2>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2],
        };

        let circle_instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<CircleInstance>() as u64,
            step_mode: wgpu::InputStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![1 => Float32x3, 2 => Float32, 3 => Float32x4],
        };

        let alpha_blend = |target| wgpu::ColorTargetState {
            format: target,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrite::ALL,
        };

        let vs_circle = device
            .create_shader_module(&wgpu::include_spirv!("../shaders/compiled/circle.vert.spv"));

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
                            HDR_FRAMEBUFFER_FORMAT.into(),
                            EFFECT_BUFFER_FORMAT.into(),
                            ignore_colour_state(EFFECT_BUFFER_FORMAT),
                        ],
                    }),
                    primitive: backface_culling,
                    depth_stencil: Some(depth_write.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            background: {
                let fs_background = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/background.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("background pipeline"),
                    layout: Some(&perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_flat_colour,
                        entry_point: "main",
                        buffers: &[background_vertex_buffer_layout.clone()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_background,
                        entry_point: "main",
                        targets: &[
                            HDR_FRAMEBUFFER_FORMAT.into(),
                            EFFECT_BUFFER_FORMAT.into(),
                            EFFECT_BUFFER_FORMAT.into(),
                        ],
                    }),
                    primitive: clamp_depth,
                    depth_stencil: Some(depth_read.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            first_bloom_blur: {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("first bloom blur pipeline"),
                    layout: Some(&bloom_blur_pipeline_layout),
                    vertex: fullscreen_tri_vertex.clone(),
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_blur,
                        entry_point: "main",
                        targets: &[additive_colour_state(EFFECT_BUFFER_FORMAT)],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            second_bloom_blur: {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("second bloom blur pipeline"),
                    layout: Some(&bloom_blur_pipeline_layout),
                    vertex: fullscreen_tri_vertex.clone(),
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_blur,
                        entry_point: "main",
                        targets: &[additive_colour_state(HDR_FRAMEBUFFER_FORMAT)],
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

                let fs_godray_blur = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/godray_blur.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("godray blur pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: fullscreen_tri_vertex.clone(),
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_godray_blur,
                        entry_point: "main",
                        targets: &[additive_colour_state(HDR_FRAMEBUFFER_FORMAT)],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            lines: {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("lines pipeline"),
                    layout: Some(&perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_flat_colour,
                        entry_point: "main",
                        buffers: &[background_vertex_buffer_layout],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(depth_write.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            bounding_boxes: {
                let vs_bounding_box = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/bounding_box.vert.spv"
                ));

                let instance_buffer_layout = wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::InputStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x3, 5 => Float32x3, 6 => Float32],
                };

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("bounding boxes pipeline"),
                    layout: Some(&perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_bounding_box,
                        entry_point: "main",
                        buffers: &[
                            vec3_vertex_buffer_layout.clone(),
                            instance_buffer_layout.clone(),
                        ],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(depth_write.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            tonemapper: {
                let pipeline_layout =
                    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                        label: Some("tonemapper pipeline layout"),
                        bind_group_layouts: &[&resources.effect_bgl],
                        push_constant_ranges: &[wgpu::PushConstantRange {
                            stages: wgpu::ShaderStage::FRAGMENT,
                            range: 0..std::mem::size_of::<colstodian::LottesTonemapper>() as u32,
                        }],
                    });

                let fs_tonemap = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/tonemap.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("tonemapper pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: fullscreen_tri_vertex,
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_tonemap,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: Some(depth_ignore),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            circle: {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("circle pipeline"),
                    layout: Some(&perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_circle,
                        entry_point: "main",
                        buffers: &[
                            vec2_vertex_buffer_layout.clone(),
                            circle_instance_buffer_layout.clone(),
                        ],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[alpha_blend(display_format)],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: Some(depth_read.clone()),
                    multisample: wgpu::MultisampleState::default(),
                })
            },
            circle_outline: {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("circle outline pipeline"),
                    layout: Some(&perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_circle,
                        entry_point: "main",
                        buffers: &[
                            vec2_vertex_buffer_layout.clone(),
                            circle_instance_buffer_layout.clone(),
                        ],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour,
                        entry_point: "main",
                        targets: &[display_format.into()],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(depth_write.clone()),
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

#[derive(Debug)]
pub struct Triangle {
    a: Vec3,
    edge_b_a: Vec3,
    edge_c_a: Vec3,
}

impl Triangle {
    fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            a,
            edge_b_a: b - a,
            edge_c_a: c - a,
        }
    }
}

impl rstar::RTreeObject for Triangle {
    type Envelope = rstar::AABB<[f32; 3]>;

    // This is only called during construction so there's no need to cache the aabb.
    fn envelope(&self) -> Self::Envelope {
        let b = self.edge_b_a + self.a;
        let c = self.edge_c_a + self.a;

        let min = self.a.min_by_component(b).min_by_component(c);
        let max = self.a.max_by_component(b).max_by_component(c);
        rstar::AABB::from_corners(min.into(), max.into())
    }
}

pub struct Model {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    num_indices: u32,
    bind_group: wgpu::BindGroup,
    bounding_box_buffer: wgpu::Buffer,
    acceleration_tree: rstar::RTree<Triangle>,
    bounding_box: resources::BoundingBox,
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

    let mut bounding_boxes = gltf
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .map(|primitive| primitive.bounding_box());
    assert_eq!(bounding_boxes.clone().count(), 1);
    let bounding_box = bounding_boxes.next().unwrap();

    let acceleration_tree = rstar::RTree::bulk_load(
        indices
            .chunks(3)
            .map(|chunk| {
                Triangle::new(
                    vertices[chunk[0] as usize].position,
                    vertices[chunk[1] as usize].position,
                    vertices[chunk[2] as usize].position,
                )
            })
            .collect(),
    );

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

    let min: Vec3 = bounding_box.min.into();
    let max: Vec3 = bounding_box.max.into();
    let bounding_box = resources::BoundingBox::new(min, max);

    Ok(Model {
        vertices,
        indices,
        num_indices,
        bind_group,
        acceleration_tree,
        bounding_box,
        bounding_box_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            usage: wgpu::BufferUsage::VERTEX,
            contents: bytemuck::cast_slice(&bounding_box.corners()),
        }),
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

fn circle_vertices<const VERTICES: usize>() -> [Vec2; VERTICES] {
    let mut verts = [Default::default(); VERTICES];

    for (i, vert) in verts.iter_mut().enumerate() {
        let rad = (i as f32) / VERTICES as f32 * std::f32::consts::TAU;
        *vert = Vec2::new(rad.sin(), rad.cos());
    }

    verts
}

fn circle_line_indices<const VERTICES: usize, const INDICES: usize>() -> [u16; INDICES] {
    let mut indices = [Default::default(); INDICES];

    for i in 0..VERTICES {
        indices[i * 2] = i as u16;
        indices[i * 2 + 1] = ((i + 1) % VERTICES) as u16;
    }

    indices
}

fn circle_filled_indices<const VERTICES: usize, const INDICES: usize>() -> [u16; INDICES] {
    let mut indices = [Default::default(); INDICES];

    for i in 0..VERTICES - 2 {
        indices[i * 3] = 0;
        indices[i * 3 + 1] = (i + 1) as u16;
        indices[i * 3 + 2] = (i + 2) as u16;
    }

    indices
}
