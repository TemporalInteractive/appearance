use std::env::var;

use glam::{Vec3, Vec4};

use super::safe_div;

pub trait Vec3Extensions {
    fn max_element_idx(&self) -> usize;
}

impl Vec3Extensions for Vec3 {
    fn max_element_idx(&self) -> usize {
        if self.x > self.y {
            if self.x > self.z {
                0
            } else {
                2
            }
        } else if self.y > self.z {
            1
        } else {
            2
        }
    }
}

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
