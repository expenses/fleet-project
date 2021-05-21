use ultraviolet::Vec3;
use wgpu::util::DeviceExt;
use ray_collisions::{BoundingBox, Triangle};
use crate::gpu_structs::ModelVertex;

pub struct Model {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
    pub bind_group: wgpu::BindGroup,
    pub bounding_box_buffer: wgpu::Buffer,
    pub acceleration_tree: rstar::RTree<Triangle>,
    pub bounding_box: BoundingBox,
}

pub fn load_ship_model(
    bytes: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
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
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(sampler),
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
    let bounding_box = BoundingBox::new(min, max);

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
