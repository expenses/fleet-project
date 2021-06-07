use ultraviolet::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Boid {
    pub pos: Vec3,
    pub vel: Vec3,
    pub max_vel: f32,
    pub radius_sq: f32,
}

impl Boid {
    fn position_at(self, time: f32) -> Vec3 {
        self.pos + self.vel * time
    }

    pub fn persue(self, target: Boid, lead_factor: f32) -> Vec3 {
        let distance = (self.pos - target.pos).mag();

        let t = distance / self.max_vel * lead_factor;
        let future_pos = target.position_at(t);

        self.seek(future_pos)
    }

    pub fn evade(self, evading_from: Boid) -> Vec3 {
        let distance = (self.pos - evading_from.pos).mag();

        let t = distance / self.max_vel;
        let future_pos = evading_from.position_at(t);

        self.flee(future_pos)
    }

    pub fn seek(self, target: Vec3) -> Vec3 {
        // todo: arrival using the braking distance:
        // let arrival_distance = self.vel.mag_sq() / (2.0 / self.max_vel);

        let desired_vel = normalize_to(target - self.pos, self.max_vel);
        desired_vel - self.vel
    }

    pub fn flee(self, target: Vec3) -> Vec3 {
        let desired_vel = normalize_to(self.pos - target, self.max_vel);
        desired_vel - self.vel
    }

    pub fn avoidance(self, other: impl Iterator<Item = Boid>) -> Vec3 {
        let mut sum = Vec3::zero();

        for boid in other {
            let vector = self.pos - boid.pos;
            let distance_sq = vector.mag_sq();
            if distance_sq > 0.0 && distance_sq < (self.radius_sq + boid.radius_sq) {
                let force = normalize_to(vector, 1.0 / distance_sq.sqrt());
                sum += force;
            }
        }

        if sum != Vec3::zero() {
            let desired_vel = normalize_to(sum, self.max_vel);
            desired_vel - self.vel
        } else {
            Vec3::zero()
        }
    }
}

fn normalize_to(vec: Vec3, new_mag: f32) -> Vec3 {
    let mag = vec.mag();
    if mag == 0.0 {
        Vec3::zero()
    } else {
        vec / mag * new_mag
    }
}
