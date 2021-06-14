#[derive(Clone, Copy, Debug)]
pub enum PersonEnum {
    Civilian = 0,
    Engineer = 1,
    Researcher = 2,
}

impl PersonEnum {
    pub const COUNT: usize = 3;
    pub const ARRAY: [Self; Self::COUNT] = [Self::Civilian, Self::Engineer, Self::Researcher];

    pub fn new(engineer: bool, researcher: bool) -> Self {
        match (engineer, researcher) {
            (true, false) => Self::Engineer,
            (false, true) => Self::Researcher,
            _ => Self::Civilian,
        }
    }
}

pub struct Engineer;
pub struct Researcher;
