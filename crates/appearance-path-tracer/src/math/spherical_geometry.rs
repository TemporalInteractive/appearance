use core::f32::consts::PI;

use glam::Vec3;

use super::{safe_acos, sqr};

pub fn spherical_triangle_area(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (2.0 * (b.cross(c).dot(a)).atan2(1.0 + a.dot(b) + a.dot(c) + b.dot(c))).abs()
}

pub fn spherical_direction(sin_theta: f32, cos_theta: f32, phi: f32) -> Vec3 {
    let clamped_sin_theta = sin_theta.clamp(-1.0, 1.0);
    let clamped_cos_theta = cos_theta.clamp(-1.0, 1.0);

    Vec3::new(
        clamped_sin_theta * phi.cos(),
        clamped_sin_theta * phi.sin(),
        clamped_cos_theta,
    )
}

pub fn spherical_theta(v: Vec3) -> f32 {
    safe_acos(v.z)
}

pub fn spherical_phi(v: Vec3) -> f32 {
    let p = v.y.atan2(v.x);
    if p < 0.0 {
        p + 2.0 * PI
    } else {
        p
    }
}

pub fn cos_theta(w: Vec3) -> f32 {
    w.z
}

pub fn cos_2_theta(w: Vec3) -> f32 {
    sqr(w.z)
}

pub fn abs_cos_theta(w: Vec3) -> f32 {
    w.z.abs()
}

pub fn sin_2_theta(w: Vec3) -> f32 {
    (1.0 - cos_2_theta(w)).max(0.0)
}

pub fn sin_theta(w: Vec3) -> f32 {
    sin_2_theta(w).sqrt()
}

pub fn tan_theta(w: Vec3) -> f32 {
    sin_theta(w) / cos_theta(w)
}

pub fn tan_2_theta(w: Vec3) -> f32 {
    sin_2_theta(w) / cos_2_theta(w)
}

pub fn cos_phi(w: Vec3) -> f32 {
    let sin_theta = sin_theta(w);
    if sin_theta == 0.0 {
        1.0
    } else {
        (w.x / sin_theta).clamp(-1.0, 1.0)
    }
}

pub fn sin_phi(w: Vec3) -> f32 {
    let sin_theta = sin_theta(w);
    if sin_theta == 0.0 {
        0.0
    } else {
        (w.y / sin_theta).clamp(-1.0, 1.0)
    }
}

pub fn same_hemisphere(w: Vec3, wp: Vec3) -> bool {
    w.z * wp.z > 0.0
}
