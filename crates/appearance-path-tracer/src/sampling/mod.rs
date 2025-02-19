use core::{
    f32::consts::{FRAC_1_PI, FRAC_PI_2, FRAC_PI_4},
    fmt::Debug,
};
use std::f32::consts::PI;

use glam::{UVec2, Vec2, Vec3};

use crate::math::{safe_sqrt, sqr};

pub mod filter;
pub mod independent_sampler;
pub mod piecewise_constant;
pub mod zsobol_sampler;

pub trait Sampler: Debug {
    fn samples_per_pixels(&self) -> u32;
    fn start_pixel_sample(&mut self, p: UVec2, sample_idx: u32, dim: u32);
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

pub fn sample_uniform_disk_polar(u: Vec2) -> Vec2 {
    let r = u.x.sqrt();
    let theta = 2.0 * PI * u.y;
    Vec2::new(r * theta.cos(), r * theta.sin())
}

pub fn sample_cosine_hemisphere(u: Vec2) -> Vec3 {
    let d = sample_uniform_disk_concentric(u);
    let z = safe_sqrt(1.0 - sqr(d.x) - sqr(d.y));
    Vec3::new(d.x, d.y, z)
}

pub fn cosine_hemisphere_pdf(cos_theta: f32) -> f32 {
    cos_theta * FRAC_1_PI
}

pub fn unit_vector_to_panorama_coords(direction: Vec3) -> Vec2 {
    let phi = (direction.z).atan2(direction.x) + PI;
    let theta = direction.y.acos();

    Vec2::new(phi / (2.0 * PI), theta * FRAC_1_PI)
}

pub fn panorama_coords_to_unit_vector(uv: Vec2) -> Vec3 {
    let phi = uv.x * (2.0 * PI);
    let theta = uv.y / FRAC_1_PI;
    let sin_theta = theta.sin();

    let x = sin_theta * phi.cos() - PI;
    let y = theta.cos();
    let z = sin_theta * phi.sin() - PI;

    Vec3::new(x, y, z)
}
