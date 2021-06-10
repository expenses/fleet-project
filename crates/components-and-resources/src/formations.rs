use ultraviolet::Vec3;

pub struct FormationPosition {
    position: Vec3,
    free: bool
}

pub struct Formation {
    positions: Vec<FormationPosition>,
}

impl Formation {
    pub fn choose_position(&mut self, location: Vec3) -> Option<Vec3> {
        let position_index = (0 .. self.positions.len())
            .filter(|&index| self.positions[index].free)
            .map(|index| (index, (self.positions[index].position - location).mag_sq()))
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        match position_index {
            Some((index, _)) => {
                let position = &mut self.positions[index];
                position.free = false;
                Some(position.position)
            },
            None => None
        }
    }
}

