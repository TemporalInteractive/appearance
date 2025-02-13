use core::f32;

const PCG32_DEFAULT_STATE: u64 = 0x853c49e6748fea9b;
const PCG32_DEFAULT_STREAM: u64 = 0xda3e39cb94b95bdb;
const PCG32_MULT: u64 = 0x5851f42d4c957f2d;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Rng {
    state: u64,
    inc: u64,
}

impl Rng {
    pub fn new() -> Self {
        Self {
            state: PCG32_DEFAULT_STATE,
            inc: PCG32_DEFAULT_STREAM,
        }
    }

    pub fn set_sequence(&mut self, sequence_idx: u64) {
        self.set_sequence_with_offset(sequence_idx, mix_bits(sequence_idx));
    }

    pub fn set_sequence_with_offset(&mut self, sequence_idx: u64, offset: u64) {
        self.state = 0;
        self.inc = (sequence_idx << 1) | 1;
        self.uniform_u32();
        self.state = self.state.wrapping_add(offset);
        self.uniform_u32();
    }

    pub fn uniform_u32(&mut self) -> u32 {
        let old_state = self.state;
        self.state = old_state.wrapping_mul(PCG32_MULT).wrapping_add(self.inc);
        let xor_shifted = (((old_state >> 18u32) ^ old_state) >> 27u32) as u32;
        let rot = (old_state >> 59u32) as u32;
        (xor_shifted >> rot) | (xor_shifted << ((!rot).wrapping_add(1u32) & 31))
    }

    pub fn uniform_f32(&mut self) -> f32 {
        self.uniform_u32() as f32 * 2.328_306_4e-10_f32
    }

    pub fn advance(&mut self, delta: i64) {
        let mut cur_mult = PCG32_MULT;
        let mut cur_plus = self.inc;
        let mut acc_mult = 1u64;
        let mut acc_plus = 0u64;
        let mut delta = delta as u64;

        while delta > 0 {
            if (delta & 1) > 0 {
                acc_mult *= cur_mult;
                acc_plus = acc_plus * cur_mult + cur_plus;
            }

            cur_plus *= cur_mult + 1;
            cur_mult *= cur_mult;
            delta /= 2;
        }
        self.state = acc_mult * self.state + acc_plus;
    }
}

// Source: http://zimbry.blogspot.ch/2011/09/better-bit-mixing-improving-on.html
pub fn mix_bits(mut v: u64) -> u64 {
    v ^= v >> 31;
    v = v.wrapping_mul(0x7fb5d329728ea185);
    v ^= v >> 27;
    v = v.wrapping_mul(0x81dadef4bc2dd44d);
    v ^= v >> 33;
    v
}

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
