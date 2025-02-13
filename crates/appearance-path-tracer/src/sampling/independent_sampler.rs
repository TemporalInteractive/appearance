use glam::{IVec2, Vec2};
use murmurhash3::murmurhash3_x64_128;

use crate::math::random::Rng;

use super::Sampler;

#[derive(Debug, Clone)]
pub struct IndependentSampler {
    samples_per_pixel: u32,
    seed: u32,
    rng: Rng,
}

impl IndependentSampler {
    pub fn new(samples_per_pixel: u32, seed: u32) -> Self {
        Self {
            samples_per_pixel,
            seed,
            rng: Rng::new(),
        }
    }
}

impl Sampler for IndependentSampler {
    fn samples_per_pixels(&self) -> u32 {
        self.samples_per_pixel
    }

    fn start_pixel_sample(&mut self, p: IVec2, sample_idx: u32, dim: u32) {
        let hash = murmurhash3_x64_128(
            &[bytemuck::bytes_of(&p), bytemuck::bytes_of(&self.seed)].concat(),
            0,
        )
        .0;
        self.rng.set_sequence(hash);
        self.rng.advance((sample_idx * 65536 + dim) as i64);
    }

    fn get_1d(&mut self) -> f32 {
        self.rng.uniform_f32()
    }

    fn get_2d(&mut self) -> Vec2 {
        Vec2::new(self.rng.uniform_f32(), self.rng.uniform_f32())
    }
}
