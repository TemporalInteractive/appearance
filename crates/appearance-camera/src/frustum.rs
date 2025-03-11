use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane(Vec4);

impl Plane {
    pub fn new(x: Vec4) -> Self {
        Self(x)
    }

    pub fn distance(&self, p: Vec3) -> f32 {
        self.0.xyz().dot(p) - self.0.w
    }
}

impl From<Plane> for Vec4 {
    fn from(val: Plane) -> Self {
        val.0
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FrustumSide {
    Left,
    Right,
    Bottom,
    Top,
    Near,
    Far,
}

pub struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    pub fn new(projection: &Mat4) -> Self {
        let m = projection.transpose();

        let planes = [
            Plane::new(m.col(3) + m.col(0)),
            Plane::new(m.col(3) - m.col(0)),
            Plane::new(m.col(3) + m.col(1)),
            Plane::new(m.col(3) - m.col(1)),
            Plane::new(m.col(3) + m.col(2)),
            Plane::new(m.col(3) - m.col(2)),
        ];

        Self { planes }
    }

    pub fn get_plane(&self, side: FrustumSide) -> Plane {
        self.planes[side as usize]
    }
}
