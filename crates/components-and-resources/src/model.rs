use crate::gpu_structs::ModelVertex;
use crate::texture_manager::TextureManager;
use ray_collisions::{BoundingBox, DynamicBvh, Triangle};
use ultraviolet::Vec3;
use wgpu::util::DeviceExt;

pub struct Model {
    pub num_indices: u32,
    pub acceleration_tree: DynamicBvh<Triangle>,
    pub bounding_box: BoundingBox,
    pub diffuse_texture: u32,
    pub emissive_texture: u32,
}

pub fn load_ship_model(
    bytes: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    merged_vertices: &mut Vec<ModelVertex>,
    merged_indices: &mut Vec<u16>,
    merged_bounding_boxes: &mut Vec<Vec3>,
    texture_manager: &mut TextureManager,
) -> anyhow::Result<Model> {
    let gltf = gltf::Gltf::from_slice(bytes)?;

    let buffer_blob = gltf.blob.as_ref().unwrap();

    let mut indices = Vec::new();

    for mesh in gltf.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| {
                assert_eq!(buffer.index(), 0);
                Some(buffer_blob)
            });

            let num_vertices = merged_vertices.len() as u16;

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
                    merged_vertices.push(ModelVertex {
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

    let triangles = indices.chunks(3).map(|chunk| {
        Triangle::new(
            merged_vertices[chunk[0] as usize].position,
            merged_vertices[chunk[1] as usize].position,
            merged_vertices[chunk[2] as usize].position,
        )
    });

    let mut acceleration_tree = DynamicBvh::default();

    for triangle in triangles {
        let bbox = triangle.bounding_box();
        acceleration_tree.insert(triangle, bbox);
    }

    let num_indices = indices.len() as u32;

    merged_indices.extend_from_slice(&indices);

    let material = gltf.materials().next().unwrap();

    let diffuse_texture = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .unwrap()
        .texture();

    let diffuse_texture = load_image(&diffuse_texture.source(), buffer_blob, device, queue)?;
    let emissive_texture = material.emissive_texture().unwrap().texture();
    let emissive_texture = load_image(&emissive_texture.source(), buffer_blob, device, queue)?;

    let diffuse_texture = texture_manager.add(diffuse_texture);
    let emissive_texture = texture_manager.add(emissive_texture);

    let bounding_box = BoundingBox::new(bounding_box.min.into(), bounding_box.max.into());

    merged_bounding_boxes.extend_from_slice(&bounding_box.corners());

    Ok(Model {
        num_indices,
        acceleration_tree,
        bounding_box,
        diffuse_texture,
        emissive_texture,
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

    load_image_from_bytes(image_bytes, device, queue)
}

pub fn load_image_from_bytes(
    image_bytes: &[u8],
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<wgpu::TextureView> {
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
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            },
            &*image,
        )
        .create_view(&wgpu::TextureViewDescriptor::default()))
}
