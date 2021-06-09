use rand::Rng;
use rand::SeedableRng;
use ultraviolet::{Rotor3, Vec2, Vec3};
use wgpu::util::DeviceExt;
use winit::event::*;
use winit::event_loop::*;

mod background;

use bevy_ecs::prelude::{IntoSystem, ParallelSystemDescriptorCoercion, Stage};
use components_and_resources::gpu_structs::*;
use components_and_resources::model::{load_image_from_bytes, load_ship_model};
use components_and_resources::{
    components,
    resources::{self, StructOpt},
    texture_manager::TextureManager,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let settings = resources::Settings::from_args();

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
            features: wgpu::Features::PUSH_CONSTANTS
                | wgpu::Features::DEPTH_CLAMPING
                | wgpu::Features::SAMPLED_TEXTURE_BINDING_ARRAY
                | wgpu::Features::MULTI_DRAW_INDIRECT,
            limits: wgpu::Limits {
                max_push_constant_size: std::mem::size_of::<[ultraviolet::Mat4; 2]>() as u32,
                ..Default::default()
            },
        },
        None,
    ))?;

    let display_format = adapter.get_swap_chain_preferred_format(&surface).unwrap();
    let window_size = window.inner_size();

    let tonemapper = colstodian::tonemapper::LottesTonemapper::new(
        colstodian::tonemapper::LottesTonemapperParams {
            gray_point_in: 0.15,
            crosstalk: 10.0,
            ..Default::default()
        },
    );

    let dimensions = resources::Dimensions {
        width: window_size.width,
        height: window_size.height,
    };

    let mut rng = rand::thread_rng();
    let mut background = background::make_background(&mut rng);

    let mut sun_dir = background::uniform_sphere_distribution(&mut rng);
    sun_dir.y = sun_dir.y.abs();

    let stars = background::create_stars(&mut rng)
        .chain(background::star_points(
            sun_dir,
            250.0,
            Vec3::broadcast(2.0) * Vec3::new(1.0, 0.8, 1.0 / 3.0),
        ))
        .collect::<Vec<_>>();

    background.extend_from_slice(&stars);

    let star_system = rendering::passes::StarSystem {
        sun_dir,
        num_background_vertices: background.len() as u32,
        background_vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("background vertices"),
            contents: bytemuck::cast_slice(&background),
            usage: wgpu::BufferUsage::VERTEX,
        }),
    };

    let constants = rendering::passes::Constants {
        bounding_box_indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bounding box vertices"),
            contents: bytemuck::cast_slice(&resources::BoundingBox::INDICES),
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

    for _ in 0..500 {
        let side = rng.gen_range(0.0..1.0) > 0.5;

        let position = Vec3::new(
            rng.gen_range(-100.0..100.0) + side as u8 as f32 * 500.0,
            rng.gen_range(-100.0..100.0),
            rng.gen_range(-100.0..100.0),
        );

        let model_rng = rng.gen_range(0.0..1.0);
        let is_fighter = model_rng < 0.8;

        let crew = if !is_fighter {
            Some(world.spawn().insert(components::PersonType::Engineer).id())
        } else {
            None
        };

        let mut spawner = world.spawn();

        spawner.insert_bundle(components::base_ship_components(
            position,
            crew.map(|crew| vec![crew]).unwrap_or_default(),
        ));

        if is_fighter {
            spawner.insert_bundle(components::fighter_components(rng.gen_range(0.0..1.0)));
        } else if model_rng < 0.95 {
            let mut queue = components::BuildQueue::default();
            queue.push(components::ShipType::Fighter, 0.0);
            spawner.insert_bundle(components::carrier_components(queue));
        } else {
            spawner.insert_bundle(components::miner_components());
        };

        if !side {
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
            components::Health(1000.0),
            components::Selectable,
            components::CanBeMined::new(100.0),
        ));
    }

    world.insert_resource(resources::ShipBuffer::new(&device));
    world.insert_resource(resources::GpuBuffer::<BackgroundVertex>::new(
        &device,
        "lines",
        wgpu::BufferUsage::VERTEX,
    ));
    world.insert_resource(resources::GpuBuffer::<LaserVertex>::new(
        &device,
        "lasers",
        wgpu::BufferUsage::VERTEX,
    ));
    world.insert_resource(resources::GpuBuffer::<CircleInstance>::new(
        &device,
        "circle instances",
        wgpu::BufferUsage::VERTEX,
    ));
    world.insert_resource(resources::GpuBuffer::<RangeInstance>::new(
        &device,
        "range instances",
        wgpu::BufferUsage::VERTEX,
    ));
    world.insert_resource(resources::GpuBuffer::<Vertex2D>::new(
        &device,
        "lines 2d",
        wgpu::BufferUsage::VERTEX,
    ));

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut bounding_boxes = Vec::new();
    let mut texture_manager = TextureManager::default();

    world.insert_resource(resources::MiscTextures {
        mined_out_asteroid: texture_manager.add(load_image_from_bytes(
            &include_bytes!("../textures/mined_out_asteroid.png")[..],
            &device,
            &queue,
        )?),
    });

    let models = [
        load_ship_model(
            include_bytes!("../models/carrier.glb"),
            &device,
            &queue,
            &mut vertices,
            &mut indices,
            &mut bounding_boxes,
            &mut texture_manager,
        )?,
        load_ship_model(
            include_bytes!("../models/fighter.glb"),
            &device,
            &queue,
            &mut vertices,
            &mut indices,
            &mut bounding_boxes,
            &mut texture_manager,
        )?,
        load_ship_model(
            include_bytes!("../models/miner.glb"),
            &device,
            &queue,
            &mut vertices,
            &mut indices,
            &mut bounding_boxes,
            &mut texture_manager,
        )?,
        load_ship_model(
            include_bytes!("../models/explosion.glb"),
            &device,
            &queue,
            &mut vertices,
            &mut indices,
            &mut bounding_boxes,
            &mut texture_manager,
        )?,
        load_ship_model(
            include_bytes!("../models/asteroid.glb"),
            &device,
            &queue,
            &mut vertices,
            &mut indices,
            &mut bounding_boxes,
            &mut texture_manager,
        )?,
    ];

    let resources = rendering::Resources::new(&device, texture_manager.count());
    let pipelines = rendering::Pipelines::new(&device, &resources, display_format);

    let mut resizables = rendering::Resizables::new(
        dimensions.width,
        dimensions.height,
        display_format,
        &device,
        &surface,
        &resources,
    );

    world.insert_resource(resources::Models {
        models,
        vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("merged model vertices"),
            usage: wgpu::BufferUsage::VERTEX,
            contents: bytemuck::cast_slice(&vertices),
        }),
        indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("merged model indices"),
            usage: wgpu::BufferUsage::INDEX,
            contents: bytemuck::cast_slice(&indices),
        }),
        bounding_boxes: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("merged model bounding box vertices"),
            usage: wgpu::BufferUsage::VERTEX,
            contents: bytemuck::cast_slice(&bounding_boxes),
        }),
        bind_group: texture_manager.into_bind_group(
            &device,
            &resources.nearest_sampler,
            &resources.merged_textures_bgl,
        ),
    });

    let glyph_brush: resources::GlyphBrush = wgpu_glyph::GlyphBrushBuilder::using_font(
        wgpu_glyph::ab_glyph::FontRef::try_from_slice(include_bytes!("../TinyUnicode.ttf"))?,
    )
    .draw_cache_position_tolerance(1.0)
    .build(&device, display_format);

    world.insert_resource(glyph_brush);
    world.insert_resource(resources::GpuInterface { device, queue });
    world.insert_resource(resources::MouseState::default());
    world.insert_resource(resources::Ray::default());
    world.insert_resource(resources::ShipUnderCursor::default());
    let orbit = resources::Orbit::default();
    world.insert_resource(resources::PerspectiveView::new(
        59.0_f32.to_radians(),
        dimensions.width as f32 / dimensions.height as f32,
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
    world.insert_resource(resources::SmallRng::from_entropy());
    world.insert_resource(resources::UnitButtons::default());
    world.insert_resource(resources::SelectedButton::default());
    world.insert_resource(resources::TopLevelAccelerationStructure::default());
    world.insert_resource(resources::GlobalMinerals::default());
    world.insert_resource(settings);

    let stage_1 = bevy_ecs::schedule::SystemStage::parallel()
        // No dependencies.
        .with_system(systems::spin.system())
        .with_system(systems::kill_temporary.system())
        .with_system(systems::expand_explosions.system())
        .with_system(systems::spawn_projectiles.system())
        .with_system(systems::update_projectiles.system())
        .with_system(systems::move_camera.system())
        .with_system(systems::set_camera_following.system())
        .with_system(systems::handle_keys.system())
        .with_system(systems::remove_unloading.system())
        .with_system(systems::build_ships::<components::Friendly>.system())
        .with_system(systems::build_ships::<components::Enemy>.system())
        .with_system(systems::debug_watch.system())
        .with_system(
            systems::apply_staging_velocity
                .system()
                .label("staging vel"),
        )
        .with_system(
            systems::apply_velocity
                .system()
                .label("vel")
                .after("staging vel"),
        )
        .with_system(systems::spawn_projectile_from_ships::<components::Friendly>.system())
        .with_system(systems::spawn_projectile_from_ships::<components::Enemy>.system())
        .with_system(systems::count_selected.system())
        .with_system(systems::set_selected_button.system())
        .with_system(systems::repair_ships.system())
        .with_system(systems::mine.system().label("mine").after("vel"))
        // Buffer clears
        .with_system(systems::clear_ship_buffer.system())
        .with_system(systems::clear_buffer::<LaserVertex>.system())
        .with_system(systems::clear_buffer::<BackgroundVertex>.system())
        .with_system(systems::clear_buffer::<RangeInstance>.system())
        .with_system(systems::clear_buffer::<Vertex2D>.system())
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
        .with_system(systems::set_rotation_from_velocity.system().label("rot"))
        // Dependent on updated rotations.
        .with_system(
            systems::update_ship_rotation_matrix
                .system()
                .label("rot_mat")
                .after("rot"),
        )
        // Dependent on updated rotation matrices.
        .with_system(
            systems::set_world_space_bounding_box
                .system()
                .label("bbox")
                .after("pos")
                .after("rot_mat"),
        )
        .with_system(systems::create_bvh.system().label("bvh").after("bbox"))
        // Dependent on model movement.
        .with_system(
            systems::move_camera_around_following
                .system()
                .label("cam")
                .after("pos"),
        )
        .with_system(
            systems::choose_enemy_target::<components::Friendly, components::Enemy>
                .system()
                .after("pos"),
        )
        .with_system(
            systems::choose_enemy_target::<components::Enemy, components::Friendly>
                .system()
                .after("pos"),
        )
        //.flush()
        // This has to go before persuit as both use the command queue.
        .with_system(
            systems::run_avoidance
                .system()
                .label("avoidance")
                .after("bvh"),
        )
        .with_system(systems::run_persuit.system().after("avoidance"))
        .with_system(systems::run_evasion.system().after("pos"))
        .with_system(systems::debug_render_targets.system().after("pos"))
        .with_system(systems::handle_left_drag.system().after("pos"))
        // Dependent on model movement and updated matrices
        .with_system(
            systems::collide_projectiles::<components::Friendly>
                .system()
                .after("bbox"),
        )
        .with_system(
            systems::collide_projectiles::<components::Enemy>
                .system()
                .after("bbox"),
        )
        // Dependent on camera movement.
        .with_system(systems::update_ray.system().label("ray").after("cam"))
        // Dependent on an updated ray
        .with_system(
            systems::update_ray_plane_point
                .system()
                .label("ray_plane")
                .after("ray"),
        )
        // Dependent on an updated ray, positions and matrices.
        .with_system(
            systems::find_ship_under_cursor
                .system()
                .label("under")
                .after("bbox"),
        )
        // .with_system(systems::debug_find_ship_under_cursor.system())
        // Dependent on `find_ship_under_cursor_system`.
        // TODO: should ideally happen BEFORE ships are moved as the player is reacting to their last seen position onsceen.
        .with_system(systems::handle_left_click.system().after("under"))
        // Staging
        .with_system(systems::render_movement_circle.system().after("ray_plane"))
        //.with_system(systems::draw_agro_ranges.system().after("pos"))
        .with_system(systems::render_drag_box.system())
        .with_system(systems::render_model_instances.system().after("under"));

    let final_stage = bevy_ecs::schedule::SystemStage::parallel()
        .with_system(systems::handle_destruction.system())
        .with_system(systems::update_mouse_state.system())
        .with_system(systems::update_keyboard_state.system())
        .with_system(systems::increase_total_time.system())
        .with_system(systems::upload_ship_buffer.system())
        .with_system(systems::render_health.system())
        .with_system(systems::debug_render_tlas.system())
        .with_system(systems::render_buttons.system());

    let upload_buffer_stage = bevy_ecs::schedule::SystemStage::parallel()
        .with_system(systems::upload_buffer::<LaserVertex>.system())
        .with_system(systems::upload_buffer::<BackgroundVertex>.system())
        .with_system(systems::upload_buffer::<RangeInstance>.system())
        .with_system(systems::upload_buffer::<Vertex2D>.system())
        .with_system(systems::upload_buffer::<CircleInstance>.system());

    let mut schedule = bevy_ecs::schedule::Schedule::default()
        .with_stage("stage 1", stage_1)
        .with_stage_after("stage 1", "stage 2", stage_2)
        .with_stage_after("stage 2", "stage 3", stage_3)
        .with_stage_after("stage 3", "final stage", final_stage)
        .with_stage_after("final stage", "buffer upload stage", upload_buffer_stage);

    /*
    let mut init_stage =
        bevy_ecs::schedule::SystemStage::parallel().with_system(systems::create_bvh.system());

    init_stage.run(&mut world);
    */

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
                    59.0_f32.to_radians(),
                    size.width as f32 / size.height as f32,
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
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -*y,
                    MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition { y, .. }) => {
                        *y as f32 / -200.0
                    }
                };

                let mut orbit = world.get_resource_mut::<resources::Orbit>().unwrap();

                orbit.zoom(delta);
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

                    if let resources::MouseMode::Movement { plane_y, .. } = &mut *mouse_mode {
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
