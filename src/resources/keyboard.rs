use crate::resources::{Camera, Orbit};
use ultraviolet::Vec3;
use winit::event::VirtualKeyCode;

#[derive(Default)]
pub struct KeyboardState {
    pub camera_forwards: bool,
    pub camera_left: bool,
    pub camera_back: bool,
    pub camera_right: bool,
    pub center_camera: Tapped,
    pub fire: bool,
    pub shift: bool,
}

#[derive(Default)]
pub struct Tapped(pub bool);

impl Tapped {
    fn handle(&mut self, pressed: bool) {
        self.0 |= pressed;
    }

    fn reset(&mut self) {
        self.0 = false;
    }
}

impl KeyboardState {
    pub fn handle(&mut self, key: VirtualKeyCode, pressed: bool) {
        match key {
            VirtualKeyCode::W => self.camera_forwards = pressed,
            VirtualKeyCode::A => self.camera_left = pressed,
            VirtualKeyCode::S => self.camera_back = pressed,
            VirtualKeyCode::D => self.camera_right = pressed,
            VirtualKeyCode::C => self.center_camera.handle(pressed),
            VirtualKeyCode::F => self.fire = pressed,
            VirtualKeyCode::LShift => self.shift = pressed,
            _ => {}
        }
    }

    pub fn update(&mut self) {
        self.center_camera.reset();
    }

    pub fn move_camera(&self, camera: &mut Camera, orbit: &Orbit) -> bool {
        let forwards = self.camera_forwards as i8 - self.camera_back as i8;
        let right = self.camera_right as i8 - self.camera_left as i8;

        if forwards != 0 || right != 0 {
            let forwards = forwards as f32;
            let right = right as f32;

            camera.center -= Vec3::new(
                forwards * orbit.latitude.sin() - right * orbit.latitude.cos(),
                0.0,
                forwards * orbit.latitude.cos() + right * orbit.latitude.sin(),
            );

            true
        } else {
            false
        }
    }
}
