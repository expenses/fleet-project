use crate::resources::PerspectiveView;
use ultraviolet::{Isometry3, Vec2, Vec3, Vec4};

#[derive(Debug, Default, Clone)]
pub struct Ray {
    origin: Vec3,
    direction: Vec3,
}

impl Ray {
    // https://antongerdelan.net/opengl/raycasting.html
    pub fn new_from_screen(
        mouse_position: Vec2,
        width: u32,
        height: u32,
        origin: Vec3,
        perspective_view: &PerspectiveView,
    ) -> Self {
        let x = (mouse_position.x / width as f32 * 2.0) - 1.0;
        let y = 1.0 - (mouse_position.y / height as f32 * 2.0);

        let clip = Vec4::new(x, y, -1.0, 1.0);
        let eye = perspective_view.perspective.inversed() * clip;
        let eye = Vec4::new(eye.x, eye.y, -1.0, 0.0);

        let direction = (perspective_view.view.inversed() * eye)
            .truncated()
            .normalized();

        Self { origin, direction }
    }

    pub fn get_intersection_point(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn center_around_transform(&mut self, transform: Isometry3) {
        let inversed = transform.inversed();

        self.origin = inversed * self.origin;
        self.direction = inversed.rotation * self.direction;
    }

    pub fn plane_intersection(&self, plane_y: f32) -> Option<f32> {
        if (self.origin.y > plane_y && self.direction.y > 0.0)
            || (self.origin.y < plane_y && self.direction.y < 0.0)
        {
            return None;
        }

        let y_delta = plane_y - self.origin.y;
        let t = y_delta / self.direction.y;

        Some(t)
    }

    // https://tavianator.com/2011/ray_box.html
    pub fn bounding_box_intersection(&self, min: Vec3, max: Vec3) -> Option<f32> {
        let ts_1 = (min - self.origin) / self.direction;
        let ts_2 = (max - self.origin) / self.direction;

        let t_mins = ts_1.min_by_component(ts_2);
        let t_maxs = ts_1.max_by_component(ts_2);

        let t_min = t_mins.component_max();
        let t_max = t_maxs.component_min();

        if t_max >= t_min {
            Some(t_min)
        } else {
            None
        }
    }
}
