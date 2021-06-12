use super::*;

#[derive(Default)]
pub struct BuildQueue {
    building: VecDeque<ShipType>,
    time_of_next_pop: f32,
    pub stay_carried: bool,
}

impl BuildQueue {
    pub fn advance(&mut self, total_time: f32) -> Option<ShipType> {
        if let Some(building) = self.building.front().copied() {
            if total_time > self.time_of_next_pop {
                self.building.pop_front();

                if let Some(next) = self.building.front().copied() {
                    self.time_of_next_pop = total_time + next.build_time();
                }

                return Some(building);
            }
        }

        None
    }

    pub fn progress_time(&self, total_time: f32) -> Option<f32> {
        if let Some(building) = self.building.front().copied() {
            let remaining = self.time_of_next_pop - total_time;
            Some(1.0 - (remaining / building.build_time()))
        } else {
            None
        }
    }

    pub fn push(&mut self, to_build: ShipType, total_time: f32) {
        if self.building.is_empty() {
            self.time_of_next_pop = total_time + to_build.build_time();
        }

        self.building.push_back(to_build);
    }

    pub fn queue_length(&self, total_time: f32) -> f32 {
        let mut sum = self
            .building
            .iter()
            .skip(1)
            .map(|model_id| model_id.build_time())
            .sum();

        if !self.building.is_empty() {
            let remaining = self.time_of_next_pop - total_time;
            sum += remaining;
        }

        sum
    }

    pub fn num_in_queue(&self) -> usize {
        self.building.len()
    }
}

#[test]
fn test_build_queue() {
    let mut build_queue = BuildQueue::default();
    build_queue.push(ShipType::Fighter, 0.0);
    assert_eq!(build_queue.progress_time(0.0), Some(0.0));
    assert_eq!(build_queue.progress_time(2.5), Some(0.5));
    assert_eq!(build_queue.progress_time(5.0), Some(1.0));
    build_queue.push(ShipType::Fighter, 0.0);
    assert_eq!(build_queue.queue_length(2.5), 7.5);
}
