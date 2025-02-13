use core::{
    clone::Clone,
    f32::consts::{FRAC_1_PI, FRAC_PI_2, FRAC_PI_4},
    fmt::Debug,
};
use std::f32::consts::PI;

use glam::{IVec2, Vec2, Vec3};

use crate::math::{safe_sqrt, sqr};

pub mod filter;
pub mod independent_sampler;

pub trait Sampler: Debug + Clone {
    fn samples_per_pixels(&self) -> u32;
    fn start_pixel_sample(&mut self, p: IVec2, sample_idx: u32, dim: u32);
    fn get_1d(&mut self) -> f32;
    fn get_2d(&mut self) -> Vec2;
}

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
