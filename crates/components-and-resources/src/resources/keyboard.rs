use winit::event::VirtualKeyCode;
use winit::window::Fullscreen;
use winit::window::Window;

pub struct KeyBindings {
    pub camera_forwards: VirtualKeyCode,
    pub camera_left: VirtualKeyCode,
    pub camera_back: VirtualKeyCode,
    pub camera_right: VirtualKeyCode,
    pub center_camera: VirtualKeyCode,
    pub fire: VirtualKeyCode,
    pub shift: VirtualKeyCode,
    pub stop: VirtualKeyCode,
    pub pause: VirtualKeyCode,
    pub unload: VirtualKeyCode,
    pub attack_move: VirtualKeyCode,
    pub escape: VirtualKeyCode,
    pub load: VirtualKeyCode,
    pub build_fighter: VirtualKeyCode,
    pub build_miner: VirtualKeyCode,
    pub build_carrier: VirtualKeyCode,
    pub toggle_fullscreen: VirtualKeyCode,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self {
            camera_forwards: VirtualKeyCode::Up,
            camera_left: VirtualKeyCode::Left,
            camera_back: VirtualKeyCode::Down,
            camera_right: VirtualKeyCode::Right,
            center_camera: VirtualKeyCode::C,
            fire: VirtualKeyCode::F,
            shift: VirtualKeyCode::LShift,
            stop: VirtualKeyCode::S,
            pause: VirtualKeyCode::P,
            unload: VirtualKeyCode::U,
            attack_move: VirtualKeyCode::A,
            escape: VirtualKeyCode::Escape,
            load: VirtualKeyCode::L,
            build_fighter: VirtualKeyCode::B,
            build_miner: VirtualKeyCode::N,
            build_carrier: VirtualKeyCode::M,
            toggle_fullscreen: VirtualKeyCode::F11,
        }
    }
}

#[derive(Default)]
pub struct KeyboardState {
    pub camera_forwards: bool,
    pub camera_left: bool,
    pub camera_back: bool,
    pub camera_right: bool,
    pub center_camera: Tapped,
    pub fire: bool,
    pub shift: bool,
    pub stop: Tapped,
    pub pause: Tapped,
    pub unload: Tapped,
    pub attack_move: Tapped,
    pub escape: Tapped,
    pub load: Tapped,
    pub build_fighter: Tapped,
    pub build_miner: Tapped,
    pub build_carrier: Tapped,
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
    #[rustfmt::skip]
    pub fn handle(&mut self, key: VirtualKeyCode, pressed: bool, window: &Window) {
        let bindings = KeyBindings::default();

        if key == bindings.camera_forwards { self.camera_forwards = pressed; }
        if key == bindings.camera_left { self.camera_left = pressed; }
        if key == bindings.camera_back { self.camera_back = pressed; }
        if key == bindings.camera_right { self.camera_right = pressed; }
        if key == bindings.center_camera { self.center_camera.handle(pressed); }
        if key == bindings.fire { self.fire = pressed; }
        if key == bindings.shift { self.shift = pressed; }
        if key == bindings.stop { self.stop.handle(pressed); }
        if key == bindings.pause { self.pause.handle(pressed); }
        if key == bindings.unload { self.unload.handle(pressed); }
        if key == bindings.attack_move { self.attack_move.handle(pressed); }
        if key == bindings.escape { self.escape.handle(pressed); }
        if key == bindings.load { self.load.handle(pressed); }
        if key == bindings.build_fighter { self.build_fighter.handle(pressed); }
        if key == bindings.build_miner { self.build_miner.handle(pressed); }
        if key == bindings.build_carrier { self.build_carrier.handle(pressed); }

        if key == bindings.toggle_fullscreen && pressed {
            if window.fullscreen().is_some() {
                window.set_fullscreen(None);
            } else {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)))
            }
        }
    }

    pub fn update(&mut self) {
        self.center_camera.reset();
        self.stop.reset();
        self.pause.reset();
        self.unload.reset();
        self.escape.reset();
        self.attack_move.reset();
        self.load.reset();

        self.build_fighter.reset();
        self.build_miner.reset();
        self.build_carrier.reset();
    }
}
