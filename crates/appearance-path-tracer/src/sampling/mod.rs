use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use crate::math::{safe_sqrt, sqr};

pub mod filter;

pub fn sample_uniform_hemisphere(u: Vec2) -> Vec3 {
    let z = u.x;
    let r = safe_sqrt(1.0 - sqr(z));
    let phi = 2.0 * PI * u.y;

    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}
