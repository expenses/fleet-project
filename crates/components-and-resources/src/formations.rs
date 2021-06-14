use rand::{rngs::SmallRng, SeedableRng};
use ultraviolet::Vec3;

#[derive(Debug, Clone, Copy)]
pub struct FormationPosition {
    position: Vec3,
    free: bool,
}

impl FormationPosition {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            free: true,
        }
    }
}

#[derive(Debug)]
pub struct Formation {
    positions: Vec<FormationPosition>,
}

impl Formation {
    pub fn choose_position(&mut self, location: Vec3) -> Option<Vec3> {
        let position_index = (0..self.positions.len())
            .filter(|&index| self.positions[index].free)
            .map(|index| (index, (self.positions[index].position - location).mag_sq()))
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        match position_index {
            Some((index, _)) => {
                let position = &mut self.positions[index];
                position.free = false;
                Some(position.position)
            }
            None => None,
        }
    }

    pub fn fighter_screen(position: Vec3, normal: Vec3, count: usize, seperation: f32) -> Self {
        let rotor = crate::utils::rotation_from_facing(normal);

        let sideways = rotor * Vec3::unit_x() * seperation;
        let up = rotor * Vec3::unit_y() * seperation;

        let width = (count as f32).sqrt().ceil() as usize;

        let middle_x = (width - 1) as f32 / 2.0;

        let middle_y = (count as f32 / width as f32).floor() / 2.0;

        Self {
            positions: (0..count)
                .map(|i| {
                    let x = (i % width) as f32 - middle_x;
                    let y = (i / width) as f32 - middle_y;

                    FormationPosition::new(position + sideways * x + up * y)
                })
                .collect(),
        }
    }

    pub fn in_sphere(position: Vec3, count: usize) -> Self {
        // Volume of a sphere: 4/3 * pi * r ^ 3
        // Radius from volume: v ^ 1/3 / pi * 3/4

        let radius = (count as f32).powf(1.0 / 3.0) / std::f32::consts::PI * 3.0 / 4.0;
        let radius = radius * 20.0;

        let mut rng = SmallRng::seed_from_u64(0);

        Self {
            positions: (0..count)
                .map(|_| {
                    FormationPosition::new(
                        position + crate::utils::random_point_in_sphere(&mut rng) * radius,
                    )
                })
                .collect(),
        }
    }

    pub fn at_point(point: Vec3, count: usize) -> Self {
        Self {
            positions: vec![FormationPosition::new(point); count],
        }
    }
}
