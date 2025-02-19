use glam::{UVec2, Vec2};
use num::clamp;

use crate::math::{find_interval, lerp};

pub struct PiecewiseConstant1D {
    func: Vec<f32>,
    cdf: Vec<f32>,
    min: f32,
    max: f32,
    integral: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct PC1DSampleResult {
    pub value: f32,
    pub pdf: f32,
    pub offset: usize,
}

impl PiecewiseConstant1D {
    pub fn new(mut func: Vec<f32>, min: f32, max: f32) -> Self {
        let mut cdf = vec![0.0; func.len() + 1];

        for f in &mut func {
            *f = f.abs();
        }

        for i in 1..(func.len() + 1) {
            cdf[i] = cdf[i - 1] + func[i - 1] * (max - min) / func.len() as f32;
        }

        let integral = cdf[func.len()];
        if integral == 0.0 {
            for i in 1..(func.len() + 1) {
                cdf[i] = i as f32 / func.len() as f32;
            }
        } else {
            for i in 1..(func.len() + 1) {
                cdf[i] /= integral;
            }
        }

        Self {
            func,
            cdf,
            min,
            max,
            integral,
        }
    }

    pub fn integral(&self) -> f32 {
        self.integral
    }

    pub fn size(&self) -> usize {
        self.func.len()
    }

    pub fn sample(&self, u: f32) -> PC1DSampleResult {
        let offset = find_interval(self.cdf.len(), |i| self.cdf[i] <= u);
        let mut du = u - self.cdf[offset];
        if (self.cdf[offset + 1] - self.cdf[offset]) > 0.0 {
            du /= self.cdf[offset + 1] - self.cdf[offset];
        }

        let pdf = if self.integral > 0.0 {
            self.func[offset] / self.integral
        } else {
            0.0
        };

        let value = lerp(
            (offset as f32 + du) / self.func.len() as f32,
            self.min,
            self.max,
        );

        PC1DSampleResult { value, pdf, offset }
    }
}

pub struct PiecewiseConstant2D {
    min: [f32; 2],
    max: [f32; 2],
    conditional: Vec<PiecewiseConstant1D>,
    marginal: PiecewiseConstant1D,
}

#[derive(Debug, Clone, Copy)]
pub struct PC2DSampleResult {
    pub value: Vec2,
    pub pdf: f32,
    pub offset: UVec2,
}

impl PiecewiseConstant2D {
    pub fn new(func: Vec<f32>, nu: usize, nv: usize, min: [f32; 2], max: [f32; 2]) -> Self {
        debug_assert!(func.len() == nu * nv);

        let mut conditional = Vec::with_capacity(nv);
        for v in 0..nv {
            conditional.push(PiecewiseConstant1D::new(
                func[(v * nu)..(v * nu + nu)].to_vec(),
                min[0],
                max[0],
            ));
        }

        let mut marginal_func = Vec::with_capacity(nv);
        for v in 0..nv {
            marginal_func.push(conditional[v].integral());
        }
        let marginal = PiecewiseConstant1D::new(marginal_func, min[1], max[1]);

        Self {
            min,
            max,
            conditional,
            marginal,
        }
    }

    pub fn integral(&self) -> f32 {
        self.marginal.integral()
    }

    pub fn sample(&self, u: Vec2) -> PC2DSampleResult {
        let d1 = self.marginal.sample(u.y);
        let d0 = self.conditional[d1.offset].sample(u.x);

        PC2DSampleResult {
            value: Vec2::new(d0.value, d1.value),
            pdf: d0.pdf * d1.pdf,
            offset: UVec2::new(d0.offset as u32, d1.offset as u32),
        }
    }

    pub fn pdf(&self, p: Vec2) -> f32 {
        let mut p = p - Vec2::new(self.min[0], self.min[1]);
        if self.max[0] > self.min[0] {
            p.x /= self.max[0] - self.min[0];
        }
        if self.max[1] > self.min[1] {
            p.y /= self.max[1] - self.min[1];
        }

        let iu = clamp(
            (p.x * self.conditional[0].size() as f32) as usize,
            0,
            self.conditional[0].size() - 1,
        );

        let iv = clamp(
            (p.y * self.marginal.size() as f32) as usize,
            0,
            self.marginal.size() - 1,
        );

        self.conditional[iv].func[iu] / self.marginal.integral()
    }
}
