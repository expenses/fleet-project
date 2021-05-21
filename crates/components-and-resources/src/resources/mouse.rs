use ultraviolet::Vec2;

#[derive(Default)]
pub struct MouseState {
    pub position: Vec2,
    pub left_state: MouseButtonState,
    pub right_state: MouseButtonState,
}

#[derive(Debug, Clone)]
pub enum MouseButtonState {
    Dragging(Vec2),
    Dragged(Vec2),
    Up,
    Clicked,
    Down(f32, Vec2),
}

impl Default for MouseButtonState {
    fn default() -> Self {
        Self::Up
    }
}

impl MouseButtonState {
    pub fn update(&mut self, delta_time: f32, drag_threshold: f32) {
        match *self {
            Self::Clicked => *self = Self::Up,
            Self::Down(ref mut time_down, start) => {
                let drag = *time_down >= drag_threshold;

                if drag {
                    *self = Self::Dragging(start)
                } else {
                    *time_down += delta_time;
                }
            }
            Self::Dragged(_) => *self = Self::Up,
            Self::Up | Self::Dragging(_) => {}
        }
    }

    pub fn handle(&mut self, mouse: Vec2, pressed: bool) {
        if pressed {
            self.handle_down(mouse);
        } else {
            self.handle_up();
        }
    }

    fn handle_down(&mut self, mouse: Vec2) {
        *self = Self::Down(0.0, mouse)
    }

    fn handle_up(&mut self) {
        match *self {
            Self::Down(_, _) => *self = Self::Clicked,
            Self::Dragging(start) => *self = Self::Dragged(start),
            _ => *self = Self::Up,
        }
    }

    pub fn was_clicked(&self) -> bool {
        matches!(self, Self::Clicked)
    }

    pub fn is_being_dragged(&self) -> Option<Vec2> {
        if let Self::Dragging(start) = self {
            Some(*start)
        } else {
            None
        }
    }

    pub fn was_dragged(&self) -> Option<Vec2> {
        if let Self::Dragged(start) = self {
            Some(*start)
        } else {
            None
        }
    }
}
