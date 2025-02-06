use std::env::var;

use glam::Vec4;

use super::safe_div;

pub trait Vec4Extensions {
    fn safe_div(&self, other: Vec4) -> Vec4;
    fn avg(&self) -> f32;
}

impl Vec4Extensions for Vec4 {
    fn safe_div(&self, other: Vec4) -> Vec4 {
        Vec4::new(
            safe_div(self.x, other.x),
            safe_div(self.y, other.y),
            safe_div(self.z, other.z),
            safe_div(self.w, other.w),
        )
    }

    fn avg(&self) -> f32 {
        self.element_sum() / 4.0
    }
}
