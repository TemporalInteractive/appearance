use std::env::var;

use glam::{Mat3, Vec3, Vec4};

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

pub trait Mat3Extensions {
    fn linear_least_squares(a: &[Vec3], b: &[Vec3]) -> Mat3;
}

impl Mat3Extensions for Mat3 {
    fn linear_least_squares(a: &[Vec3], b: &[Vec3]) -> Mat3 {
        debug_assert_eq!(a.len(), b.len());
        let rows = a.len();

        let mut at_a = [[0.0; 3]; 3];
        let mut at_b = [[0.0; 3]; 3];

        for i in 0..3 {
            for j in 0..3 {
                for r in 0..rows {
                    at_a[i][j] += a[r][i] * a[r][j];
                    at_b[i][j] += a[r][i] * b[r][j];
                }
            }
        }

        let at_a = Mat3::from_cols_array_2d(&at_a);
        let at_b = Mat3::from_cols_array_2d(&at_b);

        let at_ai = at_a.inverse();
        (at_ai * at_b).transpose()
    }
}
