use rand::Rng;
use ultraviolet::{Rotor3, Vec2, Vec3};
use wgpu::util::DeviceExt;
use winit::event::*;
use winit::event_loop::*;

mod background;

use bevy_ecs::prelude::{IntoSystem, Stage, ParallelSystemDescriptorCoercion};
use components_and_resources::gpu_structs::*;
use components_and_resources::model::load_ship_model;
use components_and_resources::{components, resources};

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

    let resources = rendering::Resources::new(&device);
    let pipelines = rendering::Pipelines::new(&device, &resources, display_format);

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

    let mut resizables = rendering::Resizables::new(
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

    let star_system = rendering::passes::StarSystem {
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

    let constants = rendering::passes::Constants {
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
    let mut world = bevy_ecs::world::World::default();

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
            world.spawn().insert_bundle((
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
            world.spawn().insert_bundle((
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

    for _ in 0..500 {
        let side = rng.gen_range(0.0..1.0) > 0.5;

        let position = Vec3::new(
            rng.gen_range(-100.0..100.0) + side as u8 as f32 * 250.0,
            rng.gen_range(-100.0..100.0),
            rng.gen_range(-100.0..100.0),
        );

        let mut spawner = world.spawn();

        spawner.insert_bundle((
            components::Position(position),
            components::Rotation(Default::default()),
            components::RotationMatrix::default(),
            components::WorldSpaceBoundingBox::default(),
            components::FollowsCommands,
            components::Velocity(Vec3::zero()),
            components::RayCooldown(rng.gen_range(0.0..1.0)),
            components::StagingVelocity(Vec3::zero())
        ));

        if true {
            spawner.insert_bundle((components::ModelId::Fighter, components::MaxSpeed(10.0)));
        } else {
            spawner.insert_bundle((components::ModelId::Carrier, components::MaxSpeed(1.0)));
        }

        if side {
            spawner.insert(components::Friendly);
        } else {
            spawner.insert(components::Enemy);
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

        world.spawn().insert_bundle((
            components::Position(position),
            components::Rotation(rotation),
            components::RotationMatrix::default(),
            components::ModelId::Asteroid,
            components::WorldSpaceBoundingBox::default(),
            components::Spin::new(background::uniform_sphere_distribution(&mut rng)),
            components::Scale(rng.gen_range(1.0..5.0)),
            components::Indestructible,
        ));
    }

    world.insert_resource(resources::ShipBuffer::new(&device));
    world.insert_resource(resources::GpuBuffer::<BackgroundVertex>::new(
        &device,
        "lines",
        wgpu::BufferUsage::VERTEX,
    ));
    world.insert_resource(resources::GpuBuffer::<CircleInstance>::new(
        &device,
        "circle instances",
        wgpu::BufferUsage::VERTEX,
    ));
    world.insert_resource(resources::Models([
        load_ship_model(
            include_bytes!("../models/carrier.glb"),
            &device,
            &queue,
            &resources.ship_bgl,
            &resources.nearest_sampler,
        )?,
        load_ship_model(
            include_bytes!("../models/fighter.glb"),
            &device,
            &queue,
            &resources.ship_bgl,
            &resources.nearest_sampler,
        )?,
        load_ship_model(
            include_bytes!("../models/explosion.glb"),
            &device,
            &queue,
            &resources.ship_bgl,
            &resources.nearest_sampler,
        )?,
        load_ship_model(
            include_bytes!("../models/asteroid.glb"),
            &device,
            &queue,
            &resources.ship_bgl,
            &resources.nearest_sampler,
        )?,
    ]));
    world.insert_resource(resources::GpuInterface { device, queue });
    world.insert_resource(resources::MouseState::default());
    world.insert_resource(resources::Ray::default());
    world.insert_resource(resources::ShipUnderCursor::default());
    let orbit = resources::Orbit::new();
    world.insert_resource(resources::PerspectiveView::new(
        ultraviolet::projection::perspective_infinite_z_wgpu_dx(
            59.0_f32.to_radians(),
            dimensions.width as f32 / dimensions.height as f32,
            0.1,
        ),
        orbit.as_vector(),
        Vec3::zero(),
    ));
    world.insert_resource(orbit);
    world.insert_resource(dimensions);
    world.insert_resource(resources::KeyboardState::default());
    world.insert_resource(resources::Camera::default());
    world.insert_resource(resources::DeltaTime(1.0 / 60.0));
    world.insert_resource(resources::TotalTime(0.0));
    world.insert_resource(resources::RayPlanePoint::default());
    world.insert_resource(resources::AverageSelectedPosition::default());
    world.insert_resource(resources::MouseMode::Normal);
    world.insert_resource(resources::Paused(false));
    world.insert_resource(bevy_tasks::TaskPool::new());

    let stage_1 = bevy_ecs::schedule::SystemStage::parallel()
        // No dependencies.
        .with_system(systems::move_ships.system())
        .with_system(systems::spin.system())
        .with_system(systems::kill_temporary.system())
        .with_system(systems::expand_explosions.system())
        .with_system(systems::spawn_projectiles.system())
        .with_system(systems::update_projectiles.system())
        .with_system(systems::move_camera.system())
        .with_system(systems::set_camera_following.system())
        .with_system(systems::handle_keys.system())
        .with_system(systems::apply_staging_velocity.system())
        .with_system(systems::apply_velocity.system())
        .with_system(systems::spawn_projectile_from_ships::<components::Friendly>.system())
        .with_system(systems::spawn_projectile_from_ships::<components::Enemy>.system())
        // Buffer clears
        .with_system(systems::clear_ship_buffer.system())
        .with_system(systems::clear_buffer::<BackgroundVertex>.system())
        .with_system(systems::clear_buffer::<CircleInstance>.system());

    // Need to update what the camera is following.
    let stage_2 = bevy_ecs::schedule::SystemStage::parallel()
        // Dependent on updated projectiles
        .with_system(systems::render_projectiles.system())
        // Dependent on ship positions (`move_ships_system`).
        .with_system(systems::calculate_average_selected_position.system())
        //  Dependent on average ship position (`calculate_average_selected_position_system`).
        .with_system(systems::handle_right_clicks.system());

    // Flush the command buffer adding `MovingTo`s to ships.
    let stage_3 = bevy_ecs::schedule::SystemStage::parallel()
        // Dependent on `handle_right_clicks_system`.
        .with_system(systems::set_rotation_from_moving_to.system().label("rot"))
        .with_system(systems::move_ships.system().label("pos"))
        // Dependent on updated rotations.
        .with_system(systems::update_ship_rotation_matrix.system().label("rot_mat").after("rot"))
        // Dependent on updated rotation matrices.
        .with_system(systems::set_world_space_bounding_box.system().label("bbox").after("pos").after("rot_mat"))
        // Dependent on model movement.
        .with_system(systems::move_camera_around_following.system().label("cam").after("pos"))
        .with_system(systems::choose_enemy_target::<components::Friendly, components::Enemy>.system().after("pos"))
        .with_system(systems::choose_enemy_target::<components::Enemy, components::Friendly>.system().after("pos"))
        //.flush()
        .with_system(systems::run_steering.system().after("pos"))
        .with_system(systems::debug_draw_targets.system().after("pos"))
        // Dependent on model movement and updated matrices
        .with_system(systems::collide_projectiles::<components::Friendly>.system().after("bbox"))
        .with_system(systems::collide_projectiles::<components::Enemy>.system().after("bbox"))
        // Dependent on camera movement.
        .with_system(systems::update_ray.system().label("ray").after("cam"))
        // Dependent on an updated ray
        .with_system(systems::update_ray_plane_point.system().label("ray_plane").after("ray"))
        // Dependent on an updated ray, positions and matrices.
        .with_system(systems::find_ship_under_cursor.system().label("under").after("bbox"))
        // .with_system(systems::debug_find_ship_under_cursor.system())
        // Dependent on `find_ship_under_cursor_system`.
        .with_system(systems::handle_left_click.system().after("under"))
        // Staging
        .with_system(systems::render_movement_circle.system().after("ray_plane"))
        .with_system(systems::upload_instances.system().after("under"));

    let final_stage = bevy_ecs::schedule::SystemStage::parallel()
        .with_system(systems::update_mouse_state.system())
        .with_system(systems::update_keyboard_state.system())
        .with_system(systems::increase_total_time.system())
        .with_system(systems::upload_ship_buffer.system())
        .with_system(systems::upload_buffer::<BackgroundVertex>.system())
        .with_system(systems::upload_buffer::<CircleInstance>.system());

    let mut schedule = bevy_ecs::schedule::Schedule::default()
        .with_stage("stage 1", stage_1)
        .with_stage_after("stage 1", "stage 2", stage_2)
        .with_stage_after("stage 2", "stage 3", stage_3)
        .with_stage_after("stage 3", "final stage", final_stage);

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent { ref event, .. } => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::Resized(size) => {
                let mut dimensions = world.get_resource_mut::<resources::Dimensions>().unwrap();

                let (width, height) = (size.width as u32, size.height as u32);

                dimensions.width = width as u32;
                dimensions.height = height as u32;

                let gpu_interface = world.get_resource::<resources::GpuInterface>().unwrap();

                resizables = rendering::Resizables::new(
                    width,
                    height,
                    display_format,
                    &gpu_interface.device,
                    &surface,
                    &resources,
                );

                let mut perspective_view = world
                    .get_resource_mut::<resources::PerspectiveView>()
                    .unwrap();

                perspective_view.set_perspective(
                    ultraviolet::projection::perspective_infinite_z_wgpu_dx(
                        59.0_f32.to_radians(),
                        size.width as f32 / size.height as f32,
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
                let mut keyboard_state = world
                    .get_resource_mut::<resources::KeyboardState>()
                    .unwrap();
                keyboard_state.handle(*key, pressed);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mut mouse_state = world.get_resource_mut::<resources::MouseState>().unwrap();

                let pressed = *state == ElementState::Pressed;
                let position = mouse_state.position;

                match button {
                    MouseButton::Left => mouse_state.left_state.handle(position, pressed),
                    MouseButton::Right => mouse_state.right_state.handle(position, pressed),
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let keyboard_state = world.get_resource::<resources::KeyboardState>().unwrap();
                let mouse_state = world.get_resource::<resources::MouseState>().unwrap();

                let position = Vec2::new(position.x as f32, position.y as f32);
                let delta = position - mouse_state.position;

                if mouse_state.right_state.is_being_dragged().is_some() {
                    let mut orbit = world.get_resource_mut::<resources::Orbit>().unwrap();
                    orbit.rotate(delta);
                } else if keyboard_state.shift {
                    let mut mouse_mode = world.get_resource_mut::<resources::MouseMode>().unwrap();

                    if let resources::MouseMode::Movement { plane_y } = &mut *mouse_mode {
                        *plane_y -= delta.y / 10.0;
                    }
                }

                {
                    let mut mouse_state =
                        world.get_resource_mut::<resources::MouseState>().unwrap();
                    mouse_state.position = position;
                }
            }
            _ => {}
        },
        Event::MainEventsCleared => {
            schedule.run(&mut world);

            window.request_redraw();
        }
        Event::RedrawRequested(_) => {
            if let Ok(frame) = resizables.swapchain.get_current_frame() {
                let gpu_interface = world.get_resource::<resources::GpuInterface>().unwrap();

                let mut encoder =
                    gpu_interface
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("render encoder"),
                        });

                rendering::passes::run_render_passes(
                    &frame,
                    &mut encoder,
                    &resizables,
                    &pipelines,
                    &world,
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
