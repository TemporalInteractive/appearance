use core::f32::consts::{FRAC_1_PI, FRAC_PI_2, FRAC_PI_4};
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

pub fn sample_uniform_disk_concentric(u: Vec2) -> Vec2 {
    let u_offset = 2.0 * u - Vec2::ONE;
    if u_offset == Vec2::ZERO {
        u_offset
    } else {
        let (theta, r) = if u_offset.x.abs() > u_offset.y.abs() {
            (u_offset.x, FRAC_PI_4 * (u_offset.y / u_offset.x))
        } else {
            (
                u_offset.y,
                FRAC_PI_2 - FRAC_PI_4 * (u_offset.x / u_offset.y),
            )
        };

        Vec2::new(theta.cos(), theta.sin()) * r
    }
}

pub fn sample_cosine_hemisphere(u: Vec2) -> Vec3 {
    let d = sample_uniform_disk_concentric(u);
    let z = safe_sqrt(1.0 - sqr(d.x) - sqr(d.y));
    Vec3::new(d.x, d.y, z)
}

pub fn cosine_hemisphere_pdf(cos_theta: f32) -> f32 {
    cos_theta * FRAC_1_PI
}
