use ultraviolet::Vec3;

#[derive(Debug)]
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
}
