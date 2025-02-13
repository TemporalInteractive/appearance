use glam::{Vec2, Vec3};

use super::normal::Normal;

pub struct Interaction {
    pub point: Vec3,
    pub wo: Vec3,
    pub normal: Normal,
    pub uv: Vec2,
}

impl Interaction {
    pub fn new_from_point(point: Vec3) -> Self {
        Self {
            point,
            wo: Vec3::ZERO,
            normal: Normal(Vec3::ZERO),
            uv: Vec2::ZERO,
        }
    }
}

pub struct SurfaceInteraction {
    pub interaction: Interaction,
    pub dpdu: Vec3,
    pub dpdv: Vec3,
    pub dndu: Normal,
    pub dndv: Normal,
    pub shading_normal: Normal,
}
