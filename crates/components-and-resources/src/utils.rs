use rand::Rng;
use ultraviolet::{Rotor3, Vec2, Vec3};

pub fn uniform_sphere_distribution<R: Rng>(rng: &mut R) -> Vec3 {
    uniform_sphere_distribution_from_coords(rng.gen_range(0.0..1.0), rng.gen_range(0.0..1.0))
}

// http://corysimon.github.io/articles/uniformdistn-on-sphere/
pub fn uniform_sphere_distribution_from_coords(x: f64, y: f64) -> Vec3 {
    use std::f64::consts::PI;

    let theta = 2.0 * PI * x;
    let phi = (1.0 - 2.0 * y).acos();

    Vec3::new(
        (phi.sin() * theta.cos()) as f32,
        (phi.sin() * theta.sin()) as f32,
        phi.cos() as f32,
    )
}

pub fn rotation_from_facing(facing: Vec3) -> Rotor3 {
    let xz_movement = Vec2::new(facing.x, facing.z).mag();

    Rotor3::from_rotation_xz(-facing.x.atan2(facing.z))
        * Rotor3::from_rotation_yz(-facing.y.atan2(xz_movement))
}

// https://datagenetics.com/blog/january32020/index.html
pub fn random_point_in_sphere<R: Rng>(rng: &mut R) -> Vec3 {
    let on_sphere = uniform_sphere_distribution(rng);
    let distance_from_center = rng.gen_range(0.0_f32..1.0).powf(1.0 / 3.0);

    on_sphere * distance_from_center
}

pub fn compare_floats(a: f32, b: f32) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}
