use glam::Vec3;

use super::sqr;

pub struct CoordSystem {
    right: Vec3,
    up: Vec3,
    forward: Vec3,
}

impl CoordSystem {
    pub fn new(forward: Vec3) -> Self {
        let sign = forward.z.signum();
        let a = -1.0 / (sign + forward.z);
        let b = forward.x * forward.y * a;

        let up = Vec3::new(1.0 + sign * sqr(forward.x) * a, sign * b, -sign * forward.x);
        let right = Vec3::new(b, sign + sqr(forward.y) * a, -forward.y);

        Self { right, up, forward }
    }

    pub fn right(&self) -> Vec3 {
        self.right
    }

    pub fn up(&self) -> Vec3 {
        self.up
    }

    pub fn forward(&self) -> Vec3 {
        self.forward
    }
}
