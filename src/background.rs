use crate::gpu_structs::BackgroundVertex;
use rand::rngs::ThreadRng;
use rand::Rng;
use spade::delaunay::FloatDelaunayTriangulation;
use tint::Colour;
use ultraviolet::{Rotor3, Vec2, Vec3};

// https://www.redblobgames.com/x/1842-delaunay-voronoi-sphere/#delaunay
pub fn make_background(rng: &mut ThreadRng) -> Vec<BackgroundVertex> {
    let nebula_colour = Colour::new(
        rng.gen_range(0.0..360.0),
        1.0,
        rng.gen_range(0.5..1.0),
        0.75,
    )
    .from_hsv();
    let nebula_colour = Vec3::new(
        nebula_colour.red as f32,
        nebula_colour.green as f32,
        nebula_colour.blue as f32,
    );
    //let colour_mod = rng.gen_range(-0.5..1.0);

    let mut dlt = FloatDelaunayTriangulation::with_walk_locate();

    // Get the point to rotate the sphere around
    let target_point = ColouredVertex::rand(rng, Rotor3::identity(), nebula_colour);

    // Get the rotation to that point
    let rotation = Rotor3::from_rotation_between(target_point.unit_pos, Vec3::unit_z());

    for _ in 0..100 {
        dlt.insert(ColouredVertex::rand(rng, rotation, nebula_colour));
    }

    let triangles_to_fill_gap = dlt
        .edges()
        // get all edges that touch the 'infinite face'
        .filter(|edge| edge.sym().face() == dlt.infinite_face())
        // make a triangle to the target point
        .flat_map(|edge| std::array::IntoIter::new([target_point, *edge.to(), *edge.from()]));

    let vertices: Vec<_> = dlt
        .triangles()
        // flat map to vertices
        .flat_map(|face| std::array::IntoIter::new(face.as_triangle()))
        .map(|vertex| *vertex)
        // chain with gap triangles
        .chain(triangles_to_fill_gap)
        // map to game vertices
        .map(|vertex| BackgroundVertex {
            position: vertex.unit_pos * 1000.0,
            colour: vertex.colour,
        })
        // collect into vec
        .collect();

    //let ambient = avg(vertices.iter().map(|vertex| vertex.normal.into()));

    vertices
}

#[derive(PartialEq, Debug, Clone, Copy)]
struct ColouredVertex {
    unit_pos: Vec3,
    projected: Vec2,
    colour: Vec3,
}

impl ColouredVertex {
    fn rand(rng: &mut ThreadRng, rotation: Rotor3, colour: Vec3) -> Self {
        use noise::{NoiseFn, Seedable};

        let unit_pos = uniform_sphere_distribution(rng);
        let rotated_pos = rotation * unit_pos;

        let value = noise::Perlin::new()
            .set_seed(rng.gen())
            .get([
                f64::from(unit_pos.x),
                f64::from(unit_pos.y),
                f64::from(unit_pos.z),
            ])
            .max(0.0) as f32;

        Self {
            unit_pos,
            colour: colour * value,
            // calculate points stereographically projected
            projected: rotated_pos.truncated() / (1.0 - rotated_pos.z),
        }
    }
}

impl spade::PointN for ColouredVertex {
    type Scalar = f32;

    fn dimensions() -> usize {
        2
    }

    fn from_value(_: Self::Scalar) -> Self {
        unimplemented!()
    }

    fn nth(&self, index: usize) -> &Self::Scalar {
        &self.projected[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self.projected[index]
    }
}

impl spade::TwoDimensional for ColouredVertex {}

pub fn uniform_sphere_distribution(rng: &mut ThreadRng) -> Vec3 {
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

pub fn create_stars(rng: &mut ThreadRng) -> impl Iterator<Item = BackgroundVertex> + '_ {
    (0..2000).flat_map(move |_| {
        let unit_pos = uniform_sphere_distribution(rng);
        std::array::IntoIter::new(star_points(unit_pos, 1.0, Vec3::one()))
    })
}

pub fn star_points(
    unit_pos: Vec3,
    scale: f32,
    colour: Vec3,
) -> [BackgroundVertex; 6] {
    let rotation = Rotor3::from_rotation_between(Vec3::unit_y(), unit_pos);

    let mut points = [
        Vec3::new(-scale, 0.0, -scale),
        Vec3::new(scale, 0.0, -scale),
        Vec3::new(-scale, 0.0, scale),
        Vec3::new(-scale, 0.0, scale),
        Vec3::new(scale, 0.0, -scale),
        Vec3::new(scale, 0.0, scale),
    ];

    rotation.rotate_vecs(&mut points);

    let mut vertices = [BackgroundVertex::default(); 6];

    for i in 0 .. 6 {
        vertices[i] = BackgroundVertex { position: points[i] + unit_pos * 1500.0, colour };
    }

    vertices
}
