use glam::Vec3;

use super::Normal;

pub struct SurfaceInteraction {
    dpdu: Vec3,
    dpdv: Vec3,
    dndu: Normal,
    dndv: Normal,
}
