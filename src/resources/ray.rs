use crate::resources::PerspectiveView;
use crate::Triangle;
use ultraviolet::{Mat3, Vec2, Vec3, Vec4};

#[derive(Debug, Default, Clone)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub inv_direction: Vec3,
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

        Self {
            origin,
            direction,
            inv_direction: Vec3::one() / direction,
        }
    }

    pub fn get_intersection_point(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn centered_around_transform(
        &self,
        position: Vec3,
        reversed_rotation: Mat3,
        scale: f32,
    ) -> Self {
        let direction = reversed_rotation * self.direction;

        Self {
            origin: reversed_rotation * (self.origin - position) / scale,
            direction,
            inv_direction: Vec3::one() / direction,
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
    pub fn bounding_box_intersection(&self, bounding_box: BoundingBox) -> Option<f32> {
        let ts_1 = (bounding_box.min - self.origin) * self.inv_direction;
        let ts_2 = (bounding_box.max - self.origin) * self.inv_direction;

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
    #[allow(clippy::many_single_char_names)]
    pub fn triangle_intersection(&self, triangle: &Triangle) -> Option<f32> {
        let h = self.direction.cross(triangle.edge_c_a);
        let determinant = triangle.edge_b_a.dot(h);

        if determinant > -f32::EPSILON && determinant < f32::EPSILON {
            return None;
        }

        // Note: we compute the reciprocal here so we have 1 div and 3 muls instead of 3 divs.
        let inv_determinant = 1.0 / determinant;
        let s = self.origin - triangle.a;
        let u = inv_determinant * s.dot(h);

        #[allow(clippy::manual_range_contains)]
        if u < 0.0 || u > 1.0 {
            return None;
        }

        let q = s.cross(triangle.edge_b_a);
        let v = inv_determinant * self.direction.dot(q);

        // Note: U + V > 1.0 NOT v > 1.0 !!
        if v < 0.0 || (u + v) > 1.0 {
            return None;
        }

        let t = inv_determinant * triangle.edge_c_a.dot(q);

        if t > f32::EPSILON {
            Some(t)
        } else {
            None
        }
    }
}

impl std::ops::Neg for &Ray {
    type Output = Ray;

    fn neg(self) -> Ray {
        Ray {
            origin: self.origin,
            direction: -self.direction,
            inv_direction: -self.inv_direction,
        }
    }
}

impl rstar::SelectionFunctionWithData<Triangle, f32> for Ray {
    fn should_unpack_parent(&self, envelope: &rstar::AABB<[f32; 3]>) -> bool {
        let bounding_box = BoundingBox::new(envelope.lower().into(), envelope.upper().into());
        self.bounding_box_intersection(bounding_box).is_some()
    }

    fn should_unpack_leaf(&self, triangle: &Triangle) -> Option<f32> {
        self.triangle_intersection(triangle)
    }
}

pub struct Projectile {
    flipped_ray: Ray,
    velocity: f32,
}

impl Projectile {
    pub fn new(ray: &Ray, velocity: f32) -> Self {
        Self {
            flipped_ray: -ray,
            velocity,
        }
    }

    pub fn max_t(&self, delta_time: f32) -> f32 {
        self.velocity * delta_time
    }

    pub fn update(&mut self, delta_time: f32) {
        self.flipped_ray.origin -= self.flipped_ray.direction * self.max_t(delta_time);
    }

    pub fn bounding_box(&self, delta_time: f32) -> BoundingBox {
        let max_t = self.max_t(delta_time);
        let end_point = self.flipped_ray.get_intersection_point(max_t);

        BoundingBox::new_checked(self.flipped_ray.origin, end_point)
    }

    pub fn line_points(&self, trail_length: f32) -> (Vec3, Vec3) {
        (
            self.flipped_ray.origin,
            self.flipped_ray
                .get_intersection_point(self.velocity * trail_length),
        )
    }

    pub fn as_limited_ray(&self, delta_time: f32) -> LimitedRay {
        LimitedRay {
            ray: self.flipped_ray.clone(),
            max_t: self.max_t(delta_time),
            scale: 1.0,
        }
    }

    pub fn get_intersection_point(&self, t: f32) -> Vec3 {
        self.flipped_ray.get_intersection_point(t)
    }
}

pub struct LimitedRay {
    ray: Ray,
    max_t: f32,
    scale: f32,
}

impl LimitedRay {
    pub fn centered_around_transform(
        &self,
        position: Vec3,
        reversed_rotation: Mat3,
        scale: f32,
    ) -> Self {
        Self {
            ray: self
                .ray
                .centered_around_transform(position, reversed_rotation, scale),
            max_t: self.max_t,
            scale: self.scale * scale,
        }
    }
}

impl rstar::SelectionFunctionWithData<Triangle, f32> for LimitedRay {
    fn should_unpack_parent(&self, envelope: &rstar::AABB<[f32; 3]>) -> bool {
        let bounding_box = BoundingBox::new(envelope.lower().into(), envelope.upper().into());
        self.ray
            .bounding_box_intersection(bounding_box)
            .map(|t| t * self.scale)
            .filter(|&t| t <= self.max_t)
            .is_some()
    }

    fn should_unpack_leaf(&self, triangle: &Triangle) -> Option<f32> {
        self.ray
            .triangle_intersection(triangle)
            .map(|t| t * self.scale)
            .filter(|&t| t <= self.max_t)
    }
}

#[derive(Copy, Clone, Default)]
pub struct BoundingBox {
    min: Vec3,
    max: Vec3,
}

impl BoundingBox {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    fn new_checked(a: Vec3, b: Vec3) -> Self {
        Self::new(a.min_by_component(b), a.max_by_component(b))
    }

    pub fn rotate(self, matrix: Mat3) -> Self {
        let corners = self.corners();

        let mut min = matrix * corners[0];
        let mut max = min;

        #[allow(clippy::needless_range_loop)]
        for i in 1..8 {
            let point = matrix * corners[i];
            min = min.min_by_component(point);
            max = max.max_by_component(point);
        }

        Self::new(min, max)
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.min.y <= other.max.y
            && self.min.z <= other.max.z
            && self.max.x >= other.min.x
            && self.max.y >= other.min.y
            && self.max.z >= other.min.z
    }

    pub fn corners(self) -> [Vec3; 8] {
        [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }
}

impl std::ops::Add<Vec3> for BoundingBox {
    type Output = Self;

    fn add(self, adjustment: Vec3) -> Self {
        Self::new(self.min + adjustment, self.max + adjustment)
    }
}

impl std::ops::Mul<f32> for BoundingBox {
    type Output = Self;

    fn mul(self, scale: f32) -> Self {
        Self::new(self.min * scale, self.max * scale)
    }
}
