use core::{cmp::Ord, ops::Mul};

pub mod coord_system;
pub mod glam_extensions;
pub mod interaction;
pub mod lookup_table;
pub mod normal;
pub mod random;
pub mod sobol;
pub mod spherical_geometry;
pub use glam_extensions::*;

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

pub fn round_up_pow2(mut v: i32) -> i32 {
    v -= 1;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v + 1
}

// Source: https://fgiesen.wordpress.com/2009/12/13/decoding-morton-codes/
pub fn left_shift_2(mut x: u64) -> u64 {
    x &= 0xffffffff;
    x = (x ^ (x << 16)) & 0x0000ffff0000ffff;
    x = (x ^ (x << 8)) & 0x00ff00ff00ff00ff;
    x = (x ^ (x << 4)) & 0x0f0f0f0f0f0f0f0f;
    x = (x ^ (x << 2)) & 0x3333333333333333;
    x = (x ^ (x << 1)) & 0x5555555555555555;
    x
}

pub fn encode_morton_2(x: u32, y: u32) -> u64 {
    (left_shift_2(y as u64) << 1) | left_shift_2(x as u64)
}

pub fn reverse_bits_32(mut n: u32) -> u32 {
    n = n.rotate_right(16);
    n = ((n & 0x00ff00ff) << 8) | ((n & 0xff00ff00) >> 8);
    n = ((n & 0x0f0f0f0f) << 4) | ((n & 0xf0f0f0f0) >> 4);
    n = ((n & 0x33333333) << 2) | ((n & 0xcccccccc) >> 2);
    n = ((n & 0x55555555) << 1) | ((n & 0xaaaaaaaa) >> 1);
    n
}

/// A very generic function to find an interval using lambdas
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

pub fn find_interval_fast(values: &[f32], x: f32) -> usize {
    let n_values = values.len() as i32 - 2;
    let mut size = n_values;
    let mut first = 1;

    while size > 0 {
        let half = size >> 1;
        let middle = first + half;
        let pred_result = values[middle as usize] <= x;

        first = if pred_result { middle + 1 } else { first };

        size = if pred_result { size - (half + 1) } else { half };
    }

    (first - 1).clamp(0, n_values) as usize
}
