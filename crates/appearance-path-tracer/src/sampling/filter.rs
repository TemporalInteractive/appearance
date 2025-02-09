use std::fmt::Debug;

use glam::{FloatExt, Vec2};

pub struct FilterSample {
    p: Vec2,
    weight: f32,
}

pub trait Filter: Debug {
    fn evaluate(&self, p: Vec2) -> f32;
    fn integral(&self) -> f32;
    fn sample(&self, u: Vec2) -> FilterSample;
    fn radius(&self) -> Vec2;
}

pub struct FilterSampler {
    domain: Vec2,
    f: Vec<Vec<f32>>,
}

impl FilterSampler {
    pub fn new(filter: &dyn Filter) -> Self {
        let domain = -filter.radius();

        let size_y = 32 * filter.radius().y as usize;
        let size_x = 32 * filter.radius().x as usize;

        let mut f = vec![vec![0.0; size_x]; size_y];

        #[allow(clippy::needless_range_loop)]
        for y in 0..size_y {
            for x in 0..size_x {
                let p = Vec2::new(
                    (-domain.x).lerp(domain.x, (x as f32 + 0.5) / size_x as f32),
                    (-domain.y).lerp(domain.y, (y as f32 + 0.5) / size_y as f32),
                );

                f[x][y] = filter.evaluate(p);
            }
        }

        // TODO, piecewise constant 2d

        Self { domain, f }
    }
}
