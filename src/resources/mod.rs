mod gpu_buffer;
mod ray;

pub use gpu_buffer::GpuBuffer;
pub use ray::Ray;

use legion::Entity;
use ultraviolet::{Mat4, Vec2, Vec3};

pub struct GpuInterface {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Default)]
pub struct ShipUnderCursor(pub Option<Entity>);

pub struct Models {
    pub carrier: crate::Model,
}

pub struct MousePosition(pub Vec2);

pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

pub struct Orbit {
    pub longitude: f32,
    pub latitude: f32,
    distance: f32,
}

impl Orbit {
    pub fn new() -> Self {
        Self {
            longitude: 1.0,
            latitude: 0.0,
            distance: 10.0,
        }
    }

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

    pub fn as_vector(&self) -> Vec3 {
        let y = self.longitude.cos();
        let horizontal_amount = self.longitude.sin();
        let x = horizontal_amount * self.latitude.sin();
        let z = horizontal_amount * self.latitude.cos();
        Vec3::new(x, y, z) * self.distance
    }
}

#[derive(Clone)]
pub struct PerspectiveView {
    pub perspective: Mat4,
    pub view: Mat4,
    view_without_movement: Mat4,
    pub perspective_view: Mat4,
    pub perspective_view_without_movement: Mat4,
}

impl PerspectiveView {
    pub fn new(perspective: Mat4, eye: Vec3, center: Vec3) -> Self {
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

    pub fn set_perspective(&mut self, perspective: Mat4) {
        self.perspective = perspective;
        self.perspective_view = self.perspective * self.view;
        self.perspective_view_without_movement = self.perspective * self.view_without_movement;
    }

    pub fn set_view(&mut self, eye: Vec3, center: Vec3) {
        self.view = Mat4::look_at(eye + center, center, Vec3::unit_y());
        self.perspective_view = self.perspective * self.view;
        self.view_without_movement = Mat4::look_at(Vec3::zero(), -eye, Vec3::unit_y());
        self.perspective_view_without_movement = self.perspective * self.view_without_movement;
    }
}
