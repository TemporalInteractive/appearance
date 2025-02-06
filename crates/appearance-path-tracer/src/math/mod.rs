use core::{cmp::Ord, ops::Mul};

pub mod coord_system;
pub use coord_system::*;
pub mod interaction;
pub use interaction::*;
pub mod normal;
pub use normal::*;
pub mod spherical_geometry;
pub use spherical_geometry::*;
pub mod vec_extensions;
pub use vec_extensions::*;

pub fn sqr<T: Mul<Output = T> + Clone + Copy>(x: T) -> T {
    x * x
}

pub fn safe_div(x: f32, y: f32) -> f32 {
    if y == 0.0 {
        0.0
    } else {
        x / y
    }
}

pub fn safe_sqrt(x: f32) -> f32 {
    x.max(0.0).sqrt()
}

pub fn safe_asin(x: f32) -> f32 {
    x.clamp(-1.0, 1.0).asin()
}

pub fn safe_acos(x: f32) -> f32 {
    x.clamp(-1.0, 1.0).acos()
}

// TODO: remove
pub fn lerp(t: f32, a: f32, b: f32) -> f32 {
    (1.0 - t) * a + t * b
}

pub fn find_interval<F>(sz: usize, pred: F) -> usize
where
    F: Fn(usize) -> bool,
{
    let mut size = sz as i32 - 2;
    let mut first = 1;

    while size > 0 {
        let half = size >> 1;
        let middle = first + half;
        let pred_result = pred(middle as usize);

        first = if pred_result { middle + 1 } else { first };

        size = if pred_result { size - (half + 1) } else { half };
    }

    (first - 1).clamp(0, sz as i32 - 2) as usize
}
