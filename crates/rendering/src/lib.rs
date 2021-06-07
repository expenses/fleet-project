pub mod passes;

use components_and_resources::gpu_structs::*;
use components_and_resources::texture_manager::TextureManager;
use ultraviolet::{Mat4, Vec2, Vec3};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const HDR_FRAMEBUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const EFFECT_BUFFER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;

pub struct Resizables {
    pub swapchain: wgpu::SwapChain,
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
    pub fn new(
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

pub struct Resources {
    pub merged_textures_bgl: wgpu::BindGroupLayout,
    effect_bgl: wgpu::BindGroupLayout,
    pub nearest_sampler: wgpu::Sampler,
    linear_sampler: wgpu::Sampler,
}

impl Resources {
    pub fn new(device: &wgpu::Device) -> Self {
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
            merged_textures_bgl: device.create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("merged textures bind group layout"),
                    entries: &[
                        sampler(0, wgpu::ShaderStage::FRAGMENT, false),
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStage::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: Some(
                                std::num::NonZeroU32::new(TextureManager::COUNT as u32).unwrap(),
                            ),
                        },
                    ],
                },
            ),
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
    z_facing_circle_outline: wgpu::RenderPipeline,
    lines_2d: wgpu::RenderPipeline,
    lasers: wgpu::RenderPipeline,
}

impl Pipelines {
    // We use helper structs and clone them around.
    // It would be a pain to remove the clone from the last use of the struct.
    #[allow(clippy::redundant_clone)]
    pub fn new(
        device: &wgpu::Device,
        resources: &Resources,
        display_format: wgpu::TextureFormat,
    ) -> Self {
        let ship_bgl_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("ship bgl pipeline layout"),
                bind_group_layouts: &[&resources.merged_textures_bgl],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    range: 0..std::mem::size_of::<PushConstants>() as u32,
                }],
            });

        let empty_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("empty pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        let model_vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModelVertex>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
        };

        let instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::InputStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![3 => Float32x3, 4 => Float32x3, 5 => Float32x3, 6 => Float32x3, 7 => Float32x3, 8 => Float32, 9 => Float32, 10 => Uint32, 11 => Uint32],
        };

        let vertex_2d_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex2D>() as u64,
            step_mode: wgpu::InputStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
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

        let seperate_perspective_view_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("seperate perspective view pipeline layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStage::VERTEX,
                    range: 0..std::mem::size_of::<[Mat4; 2]>() as u32,
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

                let mut fs_ship_desc = wgpu::include_spirv!("../shaders/compiled/ship.frag.spv");
                // Needed for WGSL reasons
                fs_ship_desc.flags = Default::default();
                let fs_ship = device.create_shader_module(&fs_ship_desc);

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
            lasers: {
                let fs_flat_colour_bloom = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/flat_colour_bloom.frag.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("lasers pipeline"),
                    layout: Some(&perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_flat_colour,
                        entry_point: "main",
                        buffers: &[background_vertex_buffer_layout.clone()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &fs_flat_colour_bloom,
                        entry_point: "main",
                        targets: &[
                            HDR_FRAMEBUFFER_FORMAT.into(),
                            EFFECT_BUFFER_FORMAT.into(),
                            ignore_colour_state(EFFECT_BUFFER_FORMAT),
                        ],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::LineList,
                        ..Default::default()
                    },
                    depth_stencil: Some(depth_write.clone()),
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
                        buffers: &[background_vertex_buffer_layout.clone()],
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
                            range: 0
                                ..std::mem::size_of::<colstodian::tonemapper::LottesTonemapper>()
                                    as u32,
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
            z_facing_circle_outline: {
                let vs_z_facing = device.create_shader_module(&wgpu::include_spirv!(
                    "../shaders/compiled/z_facing.vert.spv"
                ));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("z facing circle outline pipeline"),
                    layout: Some(&seperate_perspective_view_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_z_facing,
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
            lines_2d: {
                let vs_2d = device
                    .create_shader_module(&wgpu::include_spirv!("../shaders/compiled/2d.vert.spv"));

                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("lines 2d pipeline"),
                    layout: Some(&empty_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &vs_2d,
                        entry_point: "main",
                        buffers: &[vertex_2d_buffer_layout.clone()],
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
