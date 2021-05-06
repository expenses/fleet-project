use crate::gpu_structs::{BackgroundVertex, Instance};
use ultraviolet::{Isometry3, Vec2, Vec3};

use legion::*;
use wgpu::util::DeviceExt;

#[derive(Default)]
pub struct ShipTransform(pub Isometry3);

impl ShipTransform {
    pub fn as_instance(&self) -> Instance {
        Instance {
            rotation: self.0.rotation.into_matrix(),
            translation: self.0.translation,
        }
    }
}

pub struct Selected;

#[system(for_each)]
#[filter(maybe_changed::<ShipTransform>())]
pub fn update_ship_instances(transform: &ShipTransform, instance: &mut Instance) {
    *instance = transform.as_instance();
    println!("Updated");
}

#[system]
pub fn clear_buffer<T: 'static + Copy + bytemuck::Pod>(#[resource] buffer: &mut GpuBuffer<T>) {
    buffer.clear();
}

#[system]
pub fn upload_buffer<T: 'static + Copy + bytemuck::Pod>(
    #[resource] buffer: &mut GpuBuffer<T>,
    #[resource] device: &wgpu::Device,
) {
    buffer.upload(device);
}

#[system(for_each)]
pub fn upload_ship_instances(
    instance: &Instance,
    #[resource] ship_instance_buffer: &mut GpuBuffer<Instance>,
) {
    ship_instance_buffer.stage(&[*instance]);
}

#[system(for_each)]
pub fn update_ship_bounding_boxes(
    entity: &Entity,
    transform: &ShipTransform,
    selected: Option<&Selected>,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &ShipUnderCursor,
) {
    let colour = if ship_under_cursor.0 == Some(*entity) {
        Some(Vec3::one())
    } else if selected.is_some() {
        Some(Vec3::unit_y())
    } else {
        None
    };

    if let Some(colour) = colour {
        let lines =
            crate::bounding_box_lines(models.carrier.bounding_box_line_points, colour, transform.0);
        lines_buffer.stage(&lines);
    }

    // For Debugging the ray transform
    /*
    lines_buffer.stage(&[
        BackgroundVertex {
            colour: Vec3::one(),
            position: ray.origin,
        },
        BackgroundVertex {
            colour: Vec3::zero(),
            position: ray.origin + ray.direction * 10.0,
        }
    ])
    */
}

#[system]
pub fn find_ship_under_cursor(
    world: &legion::world::SubWorld,
    query: &mut Query<(Entity, &ShipTransform)>,
    #[resource] ray: &crate::Ray,
    #[resource] models: &Models,
    #[resource] ship_under_cursor: &mut ShipUnderCursor,
) {
    ship_under_cursor.0 = query
        .iter(world)
        .filter_map(|(entity, transform)| {
            let mut ray = ray.clone();
            ray.center_around_transform(transform.0);
            ray.bounding_box_intersection(models.carrier.min, models.carrier.max)
                .map(|t| (entity, t))
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| *entity);
}

#[system]
pub fn update_ray(
    #[resource] dimensions: &Dimensions,
    #[resource] orbit: &crate::Orbit,
    #[resource] perspective_view: &crate::utils::PerspectiveView,
    #[resource] mouse_position: &MousePosition,
    #[resource] ray: &mut crate::Ray,
) {
    *ray = crate::Ray::new_from_screen(
        mouse_position.0,
        dimensions.width,
        dimensions.height,
        orbit.as_vector(),
        perspective_view,
    );
}

#[system]
pub fn update_ray_plane_point(
    #[resource] ray: &crate::Ray,
    #[resource] lines_buffer: &mut GpuBuffer<BackgroundVertex>,
) {
    if let Some(intersection_point) = ray
        .plane_intersection(0.0)
        .map(|t| ray.get_intersection_point(t))
    {
        lines_buffer.stage(&[
            BackgroundVertex {
                position: intersection_point + Vec3::unit_y(),
                colour: Vec3::unit_x(),
            },
            BackgroundVertex {
                position: intersection_point,
                colour: Vec3::unit_y(),
            },
        ]);
    }
}

#[derive(Default)]
pub struct ShipUnderCursor(Option<Entity>);

pub struct Models {
    pub carrier: crate::Model,
}

pub struct MousePosition(pub Vec2);

pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

pub struct GpuBuffer<T> {
    staging: Vec<T>,
    buffer: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsage,
}

impl<T: Copy + bytemuck::Pod> GpuBuffer<T> {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsage) -> Self {
        Self {
            staging: Vec::new(),
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 0,
                usage: wgpu::BufferUsage::COPY_DST | usage,
                mapped_at_creation: false,
            }),
            label,
            usage,
        }
    }

    pub fn slice(&self) -> (wgpu::BufferSlice, u32) {
        (self.buffer.slice(..), self.staging.len() as u32)
    }

    fn clear(&mut self) {
        self.staging.clear();
    }

    fn stage(&mut self, slice: &[T]) {
        self.staging.extend_from_slice(slice);
    }

    fn upload(&mut self, device: &wgpu::Device) {
        self.buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(self.label),
            contents: bytemuck::cast_slice(&self.staging),
            usage: wgpu::BufferUsage::COPY_DST | self.usage,
        });
    }
}
