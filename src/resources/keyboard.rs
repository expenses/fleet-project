use winit::event::VirtualKeyCode;
use crate::resources::{Orbit, CameraCenter};
use ultraviolet::Vec3;

#[derive(Default)]
pub struct KeyboardState {
    pub camera_forwards: bool,
    pub camera_left: bool,
    pub camera_back: bool,
    pub camera_right: bool,
}

impl KeyboardState {
    pub fn handle(&mut self, key: VirtualKeyCode, pressed: bool) {
        match key {
            VirtualKeyCode::W => self.camera_forwards = pressed,
            VirtualKeyCode::A => self.camera_left = pressed,
            VirtualKeyCode::S => self.camera_back = pressed,
            VirtualKeyCode::D => self.camera_right = pressed,
            _ => {}
        }
    }

    pub fn move_camera(&self, center: &mut CameraCenter, orbit: &Orbit) {
        let forwards = self.camera_forwards as i8 - self.camera_back as i8;
        let right = self.camera_right as i8 - self.camera_left as i8;

        let forwards = forwards as f32;
        let right = right as f32;

        center.0 -= Vec3::new(
            forwards * orbit.latitude.sin() - right * orbit.latitude.cos(),
            0.0,
            forwards * orbit.latitude.cos() + right * orbit.latitude.sin(),
        );
    }
}
