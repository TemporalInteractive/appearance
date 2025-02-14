use core::iter::Iterator;

use glam::{UVec2, Vec2};
use murmurhash3::murmurhash3_x64_128;

use crate::math::{
    encode_morton_2,
    random::{mix_bits, FastOwenScrambler},
    round_up_pow2,
    sobol::sobol_sample,
};

use super::Sampler;

const PERMUTATIONS: &[[u8; 4]; 24] = &[
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 2, 3, 1],
    [0, 3, 2, 1],
    [0, 3, 1, 2],
    [1, 0, 2, 3],
    [1, 0, 3, 2],
    [1, 2, 0, 3],
    [1, 2, 3, 0],
    [1, 3, 2, 0],
    [1, 3, 0, 2],
    [2, 1, 0, 3],
    [2, 1, 3, 0],
    [2, 0, 1, 3],
    [2, 0, 3, 1],
    [2, 3, 0, 1],
    [2, 3, 1, 0],
    [3, 1, 2, 0],
    [3, 1, 0, 2],
    [3, 2, 1, 0],
    [3, 2, 0, 1],
    [3, 0, 2, 1],
    [3, 0, 1, 2],
];

#[derive(Debug, Clone)]
pub struct ZSobolSampler {
    seed: u64,
    log2_samples_per_pixel: u32,
    n_base_4_digits: u32,
    morton_idx: u64,
    dimension: u32,
}

impl ZSobolSampler {
    pub fn new(samples_per_pixel: u32, resolution: UVec2, seed: u64) -> Self {
        let log2_samples_per_pixel = (samples_per_pixel as f32).log2() as u32;

        let res = round_up_pow2(resolution.x.max(resolution.y) as i32);
        let log4_samples_per_pixel = (log2_samples_per_pixel + 1) / 2;
        let n_base_4_digits = (res as f32).log2() as u32 + log4_samples_per_pixel;

        Self {
            seed,
            log2_samples_per_pixel,
            n_base_4_digits,
            morton_idx: 0,
            dimension: 0,
        }
    }

    fn get_sample_idx(&self) -> u64 {
        let mut sample_idx = 0u64;
        let pow_2_samples = (self.log2_samples_per_pixel & 1) != 0;
        let last_digit = if pow_2_samples { 1 } else { 0 };

        for i in (last_digit..(self.n_base_4_digits as i32 - 1 + 1)).rev() {
            let digit_shift = 2 * i - if pow_2_samples { 1 } else { 0 };
            let digit = (self.morton_idx >> digit_shift) & 3;
            let higher_digits = self.morton_idx >> (digit_shift + 2);
            let p = (mix_bits(higher_digits ^ (0x55555555u64 * self.dimension as u64)) >> 24) % 24;

            let digit = PERMUTATIONS[p as usize][digit as usize];
            sample_idx |= (digit as u64) << digit_shift;
        }

        if pow_2_samples {
            let digit = self.morton_idx & 1;
            sample_idx |= digit
                ^ (mix_bits((self.morton_idx >> 1) ^ (0x55555555u64 * self.dimension as u64)) & 1);
        }

        sample_idx
    }
}

impl Sampler for ZSobolSampler {
    fn samples_per_pixels(&self) -> u32 {
        1 << self.log2_samples_per_pixel
    }

    fn start_pixel_sample(&mut self, p: UVec2, sample_idx: u32, dim: u32) {
        self.dimension = dim;
        self.morton_idx =
            (encode_morton_2(p.x, p.y) << self.log2_samples_per_pixel) | sample_idx as u64;
    }

    fn get_1d(&mut self) -> f32 {
        let sample_idx = self.get_sample_idx();
        self.dimension += 1;

        let hash = murmurhash3_x64_128(
            &[
                bytemuck::bytes_of(&self.dimension),
                bytemuck::bytes_of(&self.seed),
            ]
            .concat(),
            0,
        )
        .0;

        sobol_sample(
            sample_idx as i64,
            0,
            Some(&FastOwenScrambler::new(hash as u32)),
        )
    }

    fn get_2d(&mut self) -> Vec2 {
        let sample_idx = self.get_sample_idx();
        self.dimension += 2;

        let hash = murmurhash3_x64_128(
            &[
                bytemuck::bytes_of(&self.dimension),
                bytemuck::bytes_of(&self.seed),
            ]
            .concat(),
            0,
        )
        .0;

        Vec2::new(
            sobol_sample(
                sample_idx as i64,
                0,
                Some(&FastOwenScrambler::new(hash as u32)),
            ),
            sobol_sample(
                sample_idx as i64,
                1,
                Some(&FastOwenScrambler::new((hash >> 32) as u32)),
            ),
        )
    }
}
