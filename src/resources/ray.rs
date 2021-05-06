use crate::resources::PerspectiveView;
use crate::Triangle;
use ultraviolet::{Mat3, Vec2, Vec3, Vec4};

#[derive(Debug, Default)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
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

    pub fn centered_around_transform(&self, position: Vec3, reversed_rotation: Mat3) -> Self {
        Self {
            origin: reversed_rotation * (self.origin - position),
            direction: reversed_rotation * self.direction,
        }
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

    // https://en.wikipedia.org/wiki/M%C3%B6ller%E2%80%93Trumbore_intersection_algorithm
    // Explained:
    // https://www.scratchapixel.com/lessons/3d-basic-rendering/ray-tracing-rendering-a-triangle/moller-trumbore-ray-triangle-intersection
    pub fn triangle_intersection(&self, triangle: &Triangle) -> Option<f32> {
        let h = self.direction.cross(triangle.edge_c_a);
        let a = triangle.edge_b_a.dot(h);

        if a > -f32::EPSILON && a < f32::EPSILON {
            return None;
        }

        // Note: we compute the reciprocal here so we have 1 div and 3 muls instead of 3 divs.
        let f = 1.0 / a;
        let s = self.origin - triangle.a;
        let u = f * s.dot(h);

        if u < 0.0 || u > 1.0 {
            return None;
        }

        let q = s.cross(triangle.edge_b_a);
        let v = f * self.direction.dot(q);

        // Note: U + V > 1.0 NOT v > 1.0 !!
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * triangle.edge_c_a.dot(q);

        if t > f32::EPSILON {
            Some(t)
        } else {
            None
        }
    }
}

impl rstar::SelectionFunctionWithData<Triangle, f32> for Ray {
    fn should_unpack_parent(&self, envelope: &rstar::AABB<[f32; 3]>) -> bool {
        self.bounding_box_intersection(envelope.lower().into(), envelope.upper().into())
            .is_some()
    }

    fn should_unpack_leaf(&self, triangle: &Triangle) -> Option<f32> {
        self.triangle_intersection(triangle)
    }
}
