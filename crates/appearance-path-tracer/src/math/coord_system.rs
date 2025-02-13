use glam::Vec3;

use appearance_transform::{FORWARD, RIGHT, UP};

use super::sqr;

pub struct CoordSystem {
    x: Vec3,
    y: Vec3,
    z: Vec3,
}

impl Default for CoordSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CoordSystem {
    pub fn new() -> Self {
        Self {
            x: RIGHT,
            y: UP,
            z: FORWARD,
        }
    }

    pub fn from_xyz(x: Vec3, y: Vec3, z: Vec3) -> Self {
        Self { x, y, z }
    }

    pub fn from_xz(x: Vec3, z: Vec3) -> Self {
        Self {
            x,
            y: z.cross(x),
            z,
        }
    }

    pub fn from_xy(x: Vec3, y: Vec3) -> Self {
        Self {
            x,
            y,
            z: x.cross(y),
        }
    }

    pub fn from_x(x: Vec3) -> Self {
        let (y, z) = Self::perpendicular(x);
        Self { x, y, z }
    }

    pub fn from_y(y: Vec3) -> Self {
        let (x, z) = Self::perpendicular(y);
        Self { x, y, z }
    }

    pub fn from_z(z: Vec3) -> Self {
        let (x, y) = Self::perpendicular(z);
        Self { x, y, z }
    }

    pub fn ws_to_frame(&self, v: Vec3) -> Vec3 {
        Vec3::new(v.dot(self.x), v.dot(self.y), v.dot(self.z))
    }

    pub fn frame_to_ws(&self, v: Vec3) -> Vec3 {
        v.x * self.x + v.y * self.y + v.z * self.z
    }

    fn perpendicular(forward: Vec3) -> (Vec3, Vec3) {
        let sign = forward.z.signum();
        let a = -1.0 / (sign + forward.z);
        let b = forward.x * forward.y * a;

        let up = Vec3::new(1.0 + sign * sqr(forward.x) * a, sign * b, -sign * forward.x);
        let right = Vec3::new(b, sign + sqr(forward.y) * a, -forward.y);

        (up, right)
    }
}
