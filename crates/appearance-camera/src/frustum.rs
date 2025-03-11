use glam::{Vec3, Vec4, Vec4Swizzles};

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
}

pub struct Frustum {
    planes: [Plane; 4],
}

impl Frustum {
    pub fn new(origin: Vec3, top_left: Vec3, top_right: Vec3, bottom_left: Vec3) -> Self {
        let left = (top_left - bottom_left).cross(top_left - origin);
        let right = (top_right - origin).cross(top_left - bottom_left);
        let top = (top_right - top_left).cross(top_left - origin);
        let bottom = (bottom_left - origin).cross(top_right - top_left);

        let planes = [
            Plane::new(Vec4::from((left, left.dot(origin)))),
            Plane::new(Vec4::from((right, right.dot(origin)))),
            Plane::new(Vec4::from((top, top.dot(origin)))),
            Plane::new(Vec4::from((bottom, bottom.dot(origin)))),
        ];

        Self { planes }
    }

    pub fn get_plane(&self, side: FrustumSide) -> Plane {
        self.planes[side as usize]
    }
}
