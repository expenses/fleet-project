use super::*;

pub fn base_ship_components(position: Vec3, crew: Vec<Entity>) -> impl Bundle {
    (
        Position(position),
        OnBoard(crew),
        Rotation(Default::default()),
        RotationMatrix::default(),
        WorldSpaceBoundingBox::default(),
        Velocity(Vec3::zero()),
        StagingPersuitForce(Vec3::zero()),
        StagingEvasionForce(Vec3::zero()),
        StagingAvoidanceForce(Vec3::zero()),
        CommandQueue::default(),
        Selectable,
    )
}

pub fn fighter_components(ray_cooldown: f32) -> impl Bundle {
    (
        ModelId::Fighter,
        CanAttack,
        CanBeCarried,
        MaxSpeed(10.0),
        Health::new(50.0),
        RayCooldown(ray_cooldown),
        AgroRange(200.0),
    )
}

pub fn miner_components() -> impl Bundle {
    (
        ModelId::Miner,
        CanBeCarried,
        MaxSpeed(15.0),
        Health::new(40.0),
        CanMine,
        StoredMinerals {
            stored: 0.0,
            capacity: 10.0,
        },
    )
}

pub fn carrier_components(queue: BuildQueue) -> impl Bundle {
    (
        ModelId::Carrier,
        Carrying::default(),
        MaxSpeed(5.0),
        Health {
            current: 247.5,
            max: 250.0,
        },
        queue,
    )
}
