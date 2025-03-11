use bytemuck::{Pod, Zeroable};
use glam::{UVec3, Vec2, Vec2Swizzles, Vec3, Vec3Swizzles};

/// Pack a hdr rgb color in a single u32
#[derive(Debug, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct PackedRgb9e5 {
    data: u32,
}

/// Pack a normalized unit vector in a single u32
#[derive(Debug, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct PackedNormalizedXyz10 {
    data: u32,
}

impl Default for PackedRgb9e5 {
    fn default() -> Self {
        Self::new(Vec3::new(1.0, 0.0, 1.0))
    }
}

impl PackedRgb9e5 {
    pub fn new(rgb: Vec3) -> Self {
        let max_val = f32::from_bits(0x477F8000u32);
        let min_val = f32::from_bits(0x37800000u32);

        let clamped_rgb = rgb.clamp(Vec3::ZERO, Vec3::splat(max_val));

        let max_channel = clamped_rgb.max_element().max(min_val);
        let max_channel_as_u32 = max_channel.to_bits();

        let bias = f32::from_bits((max_channel_as_u32.wrapping_add(0x07804000u32)) & 0x7F800000u32);
        let bias_as_u32 = bias.to_bits();

        let rgb_as_u32 = unsafe { std::mem::transmute::<Vec3, UVec3>(clamped_rgb + bias) };
        let e = (bias_as_u32 << 4u32).wrapping_add(0x10000000u32);

        Self {
            data: e | (rgb_as_u32.z << 18) | (rgb_as_u32.y << 9) | (rgb_as_u32.x & 0x1FFu32),
        }
    }
}

impl Default for PackedNormalizedXyz10 {
    fn default() -> Self {
        Self::new(Vec3::new(0.0, 1.0, 0.0))
    }
}

impl PackedNormalizedXyz10 {
    pub fn new(dir: Vec3) -> Self {
        let oct_encoded_dir = dir_oct_quad_encode(dir);
        let data = ((((oct_encoded_dir.y * (0x7fff as f32)).round()) as u32) << 15)
            | ((oct_encoded_dir.x * (0x7fff as f32)).round()) as u32;

        Self { data }
    }
}

// Inspired by https://knarkowicz.wordpress.com/2014/04/16/octahedron-normal-vector-encoding/
fn dir_oct_quad_encode(dir: Vec3) -> Vec2 {
    let mut ret_val = dir.xy() / ((dir.x).abs() + (dir.y).abs() + (dir.z).abs());
    if dir.z < 0.0 {
        let mut signs = Vec2::ZERO;
        if ret_val.x >= 0.0 {
            signs.x = 1.0;
        } else {
            signs.x = -1.0;
        }
        if ret_val.y >= 0.0 {
            signs.y = 1.0;
        } else {
            signs.y = -1.0;
        }

        ret_val = (1.0 - (ret_val.yx()).abs()) * signs;
    }
    ret_val * 0.5 + 0.5
}
