use glam::{Vec2, Vec3, Vec4};

pub fn wangh_hash(x: u32) -> u32 {
    let mut state = (x ^ 61u32) ^ (x >> 16u32);
    state *= 9u32;
    state = state ^ (state >> 4u32);
    state *= 0x27d4eb2du32;
    state ^ (state >> 15u32)
}

pub fn pcg_hash(x: u32) -> u32 {
    let state: u32 = x * 747796405u32 + 2891336453u32;
    let word: u32 = ((state >> ((state >> 28u32) + 4u32)) ^ state) * 277803737u32;
    (word >> 22u32) ^ word
}

/// Use this as a good seed to initialize the xor_shift_u32 state
pub fn splitmix_64(state: &mut u64) -> u64 {
    *state = (*state).wrapping_add(0x9E3779B97F4A7C15);
    let mut result = *state;
    result = (result ^ (result >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    result = (result ^ (result >> 27)).wrapping_mul(0x94D049BB133111EB);
    result ^ (result >> 31)
}

/// Fast high quality random number generator
pub fn xor_shift_u32(state: &mut u32) -> u32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    *state
}

/// Fast high quality random f32 generator based on the `xor_shift_u32` in the range of [0, 1]
pub fn random_f32(state: &mut u32) -> f32 {
    xor_shift_u32(state) as f32 * 2.328_306_4e-10_f32
}

/// Fast high quality random f32 generator based on the `xor_shift_u32` in the range of [lo, hi]
pub fn random_f32_ranged(state: &mut u32, lo: f32, hi: f32) -> f32 {
    let random = random_f32(state);
    lo + random * (hi - lo)
}
