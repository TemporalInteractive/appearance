use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use crate::{
    math::{
        lerp,
        spherical_geometry::{abs_cos_theta, cos_2_theta, cos_phi, sin_phi, tan_2_theta},
        sqr,
    },
    sampling::sample_uniform_disk_polar,
};

#[derive(Debug, Clone)]
pub struct ThrowbridgeReitzDistribution {
    alpha_x: f32,
    alpha_y: f32,
}

impl ThrowbridgeReitzDistribution {
    pub fn new(alpha_x: f32, alpha_y: f32) -> Self {
        Self { alpha_x, alpha_y }
    }

    pub fn d(&self, wm: Vec3) -> f32 {
        let tan_2_theta = tan_2_theta(wm);
        if tan_2_theta.is_infinite() {
            0.0
        } else {
            let cos_4_theta = sqr(cos_2_theta(wm));
            let e =
                tan_2_theta * (sqr(cos_phi(wm) / self.alpha_x) + sqr(sin_phi(wm) / self.alpha_y));
            1.0 / (PI * self.alpha_x * self.alpha_y * cos_4_theta * sqr(1.0 + e))
        }
    }

    pub fn is_smooth(&self) -> bool {
        self.alpha_x.max(self.alpha_y) < 1e-3
    }

    pub fn lambda(&self, w: Vec3) -> f32 {
        let tan_2_theta = tan_2_theta(w);
        if tan_2_theta.is_infinite() {
            0.0
        } else {
            let alpha_2 = sqr(cos_phi(w) * self.alpha_x) + sqr(sin_phi(w) * self.alpha_y);
            ((1.0 + alpha_2 * tan_2_theta).sqrt() - 1.0) / 2.0
        }
    }

    pub fn g1(&self, w: Vec3) -> f32 {
        1.0 / (1.0 + self.lambda(w))
    }

    pub fn g(&self, wo: Vec3, wi: Vec3) -> f32 {
        1.0 / (1.0 + self.lambda(wo) + self.lambda(wi))
    }

    pub fn d2(&self, w: Vec3, wm: Vec3) -> f32 {
        self.g1(w) / abs_cos_theta(w) * self.d(wm) * w.dot(wm).abs()
    }

    pub fn pdf(&self, w: Vec3, wm: Vec3) -> f32 {
        self.d2(w, wm)
    }

    pub fn sample_wm(&self, w: Vec3, u: Vec2) -> Vec3 {
        let mut wh = Vec3::new(self.alpha_x * w.x, self.alpha_y * w.y, w.z).normalize();
        if wh.z < 0.0 {
            wh = -wh;
        }

        let t1 = if wh.z < 0.99999 {
            Vec3::new(0.0, 0.0, 1.0).cross(wh).normalize()
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let t2 = wh.cross(t1);

        let mut p = sample_uniform_disk_polar(u);

        let h = (1.0 - sqr(p.x)).sqrt();
        p.y = lerp((1.0 + wh.z) / 2.0, h, p.y);

        let pz = (1.0 - p.length_squared()).max(0.0).sqrt();
        let nh = p.x * t1 + p.y * t2 + pz * wh;
        Vec3::new(self.alpha_x * nh.x, self.alpha_y * nh.y, nh.z.max(1e-6)).normalize()
    }
}
