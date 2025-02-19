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
        #[allow(clippy::needless_range_loop)]
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

impl PiecewiseConstant2D {
    pub fn new(func: Vec<f32>, nu: i32, nv: i32, min: [f32; 2], max: [f32; 2]) -> Self {
        todo!()
    }
}
