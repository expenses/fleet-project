mod glyph_layout_cache;
mod gpu_buffer;
mod keyboard;
mod mouse;

pub use glyph_layout_cache::GlyphLayoutCache;
pub use gpu_buffer::{GpuBuffer, ShipBuffer};
pub use keyboard::KeyboardState;
pub use mouse::{MouseButtonState, MouseState};
pub use rand::rngs::SmallRng;
pub use ray_collisions::{BoundingBox, DynamicBvh, Projectile, Ray, SelectionFrustum};
pub use structopt::StructOpt;

use crate::components::{ModelId, MoveType};
use crate::model::Model;
use bevy_ecs::prelude::Entity;
use ultraviolet::{Mat4, Vec2, Vec3};

#[derive(StructOpt)]
pub struct Settings {
    #[structopt(long)]
    pub disable_godrays: bool,
    #[structopt(long)]
    pub disable_bloom: bool,
    #[structopt(long)]
    pub enable_tlas_debug_drawing: bool,
}

pub struct DpiFactor(pub f32);

#[derive(Default)]
pub struct GlobalResearch(pub f32);

#[derive(Default)]
pub struct GlobalMinerals(pub f32);

pub type TopLevelAccelerationStructure = DynamicBvh<Entity>;

pub struct MiscTextures {
    pub mined_out_asteroid: u32,
}

#[derive(Default)]
pub struct UnitButtons(pub Vec<(ModelId, UnitStatus)>);

impl UnitButtons {
    pub const LINE_HEIGHT: f32 = 18.0;
    pub const BUTTON_WIDTH: f32 = 130.0;
    pub const UI_LINES: isize = 2;
}

#[derive(Default)]
pub struct SelectedButton(pub Option<usize>);

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum UnitStatus {
    Friendly { carried: bool },
    Neutral,
    Enemy,
}

impl UnitStatus {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Friendly { carried: false } => "Friendly",
            Self::Friendly { carried: true } => "Friendly (being carried)",
            Self::Neutral => "Neutral",
            Self::Enemy => "Enemy",
        }
    }

    pub fn from_bools(friendly: bool, enemy: bool, carried: bool) -> Self {
        match (friendly, enemy) {
            (true, false) => Self::Friendly { carried },
            (false, true) => Self::Enemy,
            _ => Self::Neutral,
        }
    }
}

pub struct Paused(pub bool);

pub enum MouseMode {
    Normal,
    Movement { point_on_plane: Vec3, ty: MoveType },
}

#[derive(Default)]
pub struct AverageSelectedPosition(pub Option<Vec3>);

pub struct TotalTime(pub f32);

pub struct DeltaTime(pub f32);

pub struct GpuInterface {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

#[derive(Default)]
pub struct ShipUnderCursor(pub Option<Entity>);

pub struct Models {
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub bounding_boxes: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub models: [Model; Self::COUNT],
}

impl Models {
    pub const COUNT: usize = 5;
    pub const ARRAY: [ModelId; Self::COUNT] = [
        ModelId::Carrier,
        ModelId::Fighter,
        ModelId::Miner,
        ModelId::Explosion,
        ModelId::Asteroid,
    ];

    pub const MINER_LASER_OFFSET: Vec3 = Vec3::new(0.0, 1.89621, 0.87578);

    pub fn get(&self, id: ModelId) -> &Model {
        &self.models[id as usize]
    }
}

#[derive(Default)]
pub struct Camera {
    pub center: Vec3,
}

impl Camera {
    pub fn control(
        &mut self,
        orbit: &Orbit,
        forwards: bool,
        back: bool,
        left: bool,
        right: bool,
    ) -> bool {
        let forwards = forwards as i8 - back as i8;
        let right = right as i8 - left as i8;

        if forwards != 0 || right != 0 {
            self.center += orbit.camera_movement(forwards as f32, right as f32);

            true
        } else {
            false
        }
    }
}

pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    pub fn to_vec(&self) -> Vec2 {
        Vec2::new(self.width as f32, self.height as f32)
    }
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

        let epsilon = 0.0001;

        self.latitude -= delta.x.to_radians() * speed;
        self.longitude = (self.longitude - delta.y.to_radians() * speed)
            .max(epsilon)
            .min(PI - epsilon);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance * (1.0 + delta * 0.1)).max(1.0).min(250.0);
    }

    pub fn as_vector(&self) -> Vec3 {
        let y = self.longitude.cos();
        let horizontal_amount = self.longitude.sin();
        let x = horizontal_amount * self.latitude.sin();
        let z = horizontal_amount * self.latitude.cos();
        Vec3::new(x, y, z) * self.distance
    }

    pub fn camera_movement(&self, forwards: f32, right: f32) -> Vec3 {
        -Vec3::new(
            forwards * self.latitude.sin() - right * self.latitude.cos(),
            0.0,
            forwards * self.latitude.cos() + right * self.latitude.sin(),
        )
    }
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            longitude: 1.0,
            latitude: 0.0,
            distance: 10.0,
        }
    }
}

#[derive(Clone)]
pub struct PerspectiveView {
    pub perspective: Mat4,
    perspective_with_far_plane: Mat4,
    pub view: Mat4,
    view_without_movement: Mat4,
    pub perspective_view: Mat4,
    pub perspective_view_without_movement: Mat4,
    pub perspective_view_with_far_plane: Mat4,
}

impl PerspectiveView {
    pub fn new(fov: f32, aspect_ratio: f32, eye: Vec3, center: Vec3) -> Self {
        let perspective =
            ultraviolet::projection::perspective_infinite_z_wgpu_dx(fov, aspect_ratio, 0.1);
        let perspective_with_far_plane =
            ultraviolet::projection::perspective_wgpu_dx(fov, aspect_ratio, 0.1, 1000.0);

        let view = Mat4::look_at(eye + center, center, Vec3::unit_y());
        let view_without_movement = Mat4::look_at(Vec3::zero(), -eye, Vec3::unit_y());

        Self {
            view,
            view_without_movement,
            perspective,
            perspective_with_far_plane,
            perspective_view: perspective * view,
            perspective_view_without_movement: perspective * view_without_movement,
            perspective_view_with_far_plane: perspective_with_far_plane * view,
        }
    }

    fn recalculate(&mut self) {
        self.perspective_view = self.perspective * self.view;
        self.perspective_view_without_movement = self.perspective * self.view_without_movement;
        self.perspective_view_with_far_plane = self.perspective_with_far_plane * self.view;
    }

    pub fn set_perspective(&mut self, fov: f32, aspect_ratio: f32) {
        self.perspective =
            ultraviolet::projection::perspective_infinite_z_wgpu_dx(fov, aspect_ratio, 0.1);
        self.perspective_with_far_plane =
            ultraviolet::projection::perspective_wgpu_dx(fov, aspect_ratio, 0.1, 1000.0);
        self.recalculate();
    }

    pub fn set_view(&mut self, orbit: Vec3, center: Vec3) {
        self.view = Mat4::look_at(orbit + center, center, Vec3::unit_y());
        self.view_without_movement = Mat4::look_at(Vec3::zero(), -orbit, Vec3::unit_y());
        self.recalculate();
    }
}
