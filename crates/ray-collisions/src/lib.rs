use ultraviolet::{Mat3, Mat4, Vec2, Vec3, Vec4};

mod dynamic_bvh;

pub use dynamic_bvh::DynamicBvh;

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
        inv_perspective: Mat4,
        inv_view: Mat4,
    ) -> Self {
        let point = to_wgpu_coords(mouse_position, Vec2::new(width as f32, height as f32));

        let clip = Vec4::new(point.x, point.y, -1.0, 1.0);
        let eye = inv_perspective * clip;
        let eye = Vec4::new(eye.x, eye.y, -1.0, 0.0);

        let direction = (inv_view * eye).truncated().normalized();

        Self::new(origin, direction)
    }

    #[inline]
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction,
            inv_direction: Vec3::one() / direction,
        }
    }

    #[inline]
    pub fn get_intersection_point(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn centered_around_transform(
        &self,
        position: Vec3,
        reversed_rotation: Mat3,
        scale: f32,
    ) -> Self {
        Self::new(
            reversed_rotation * (self.origin - position) / scale,
            reversed_rotation * self.direction,
        )
    }

    #[inline]
    pub fn y_plane_intersection(&self, plane_y: f32) -> Option<f32> {
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

    #[inline]
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

#[derive(Copy, Clone, Default, Debug)]
pub struct BoundingBox {
    min: Vec3,
    max: Vec3,
}

impl BoundingBox {
    #[inline]
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    fn new_checked(a: Vec3, b: Vec3) -> Self {
        Self::new(a.min_by_component(b), a.max_by_component(b))
    }

    #[inline]
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

    #[inline]
    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.min.y <= other.max.y
            && self.min.z <= other.max.z
            && self.max.x >= other.min.x
            && self.max.y >= other.min.y
            && self.max.z >= other.min.z
    }

    #[inline]
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

    #[inline]
    fn union_with(self, other: Self) -> Self {
        Self {
            min: self.min.min_by_component(other.min),
            max: self.max.max_by_component(other.max),
        }
    }

    #[inline]
    fn surface_area(self) -> f32 {
        let inner = self.max - self.min;
        2.0 * (inner.x * inner.y + inner.y * inner.z + inner.z * inner.x)
    }

    #[inline]
    pub fn expand(self, by: Vec3) -> Self {
        Self {
            min: self.min - by,
            max: self.max + by,
        }
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

#[derive(Debug)]
pub struct Triangle {
    pub a: Vec3,
    pub edge_b_a: Vec3,
    pub edge_c_a: Vec3,
}

impl Triangle {
    pub fn new(a: Vec3, b: Vec3, c: Vec3) -> Self {
        Self {
            a,
            edge_b_a: b - a,
            edge_c_a: c - a,
        }
    }
}

impl rstar::RTreeObject for Triangle {
    type Envelope = rstar::AABB<[f32; 3]>;

    // This is only called during construction so there's no need to cache the aabb.
    fn envelope(&self) -> Self::Envelope {
        let b = self.edge_b_a + self.a;
        let c = self.edge_c_a + self.a;

        let min = self.a.min_by_component(b).min_by_component(c);
        let max = self.a.max_by_component(b).max_by_component(c);
        rstar::AABB::from_corners(min.into(), max.into())
    }
}

fn to_wgpu_coords(point: Vec2, dimensions: Vec2) -> Vec2 {
    let scaled = point / dimensions * 2.0;

    Vec2::new(scaled.x - 1.0, 1.0 - scaled.y)
}

#[derive(Debug)]
struct Plane {
    normal: Vec3,
    // Distance from origin
    constant: f32,
}

impl Plane {
    fn new_from_normal_and_coplanar_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            normal,
            constant: point.dot(normal),
        }
    }

    fn new_from_3_coplanar_points(a: Vec3, b: Vec3, c: Vec3) -> Self {
        let normal = (c - b).cross(a - b).normalized();
        Self::new_from_normal_and_coplanar_point(normal, a)
    }

    fn half_space(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.constant
    }
}

#[derive(Debug)]
pub struct SelectionFrustum {
    left: Plane,
    right: Plane,
    top: Plane,
    bot: Plane,
}

impl SelectionFrustum {
    pub fn new_from_onscreen_box(
        min: Vec2,
        max: Vec2,
        screen_width: u32,
        screen_height: u32,
        inv_projection_view: Mat4,
    ) -> Self {
        let dimensions = Vec2::new(screen_width as f32, screen_height as f32);

        let to_3d = |point: Vec2, depth| {
            let point = to_wgpu_coords(point, dimensions);

            let point = inv_projection_view * Vec4::new(point.x, point.y, depth, 1.0);
            point.truncated() / point.w
        };

        let near = [
            to_3d(min, -1.0),
            to_3d(Vec2::new(max.x, min.y), -1.0),
            to_3d(Vec2::new(min.x, max.y), -1.0),
            to_3d(max, -1.0),
        ];

        let far = [
            to_3d(min, 1.0),
            to_3d(Vec2::new(max.x, min.y), 1.0),
            to_3d(Vec2::new(min.x, max.y), 1.0),
            to_3d(max, 1.0),
        ];

        Self::new_from_corners(near, far)
    }

    fn new_from_corners(near_corners: [Vec3; 4], far_corners: [Vec3; 4]) -> Self {
        /*
        let near = Plane::new_from_3_coplanar_points(
            near_corners[0], near_corners[2], near_corners[1]
        );

        let far = Plane::new_from_3_coplanar_points(
            far_corners[0], far_corners[3], far_corners[1]
        );
        */

        Self {
            left: Plane::new_from_3_coplanar_points(
                near_corners[0],
                far_corners[2],
                far_corners[0],
            ),

            top: Plane::new_from_3_coplanar_points(far_corners[1], near_corners[0], far_corners[0]),

            right: Plane::new_from_3_coplanar_points(
                near_corners[3],
                far_corners[1],
                far_corners[3],
            ),

            bot: Plane::new_from_3_coplanar_points(far_corners[3], far_corners[2], near_corners[2]),
        }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        self.left.half_space(point) >= 0.0
            && self.right.half_space(point) >= 0.0
            && self.top.half_space(point) >= 0.0
            && self.bot.half_space(point) >= 0.0
    }
}
