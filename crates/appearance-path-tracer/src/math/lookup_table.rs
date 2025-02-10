/// Create an optimized lookup table for expensive functions
pub struct LookupTable {
    values: Vec<f32>,
    precision_scale: f32,
    min_x: f32,
}

impl LookupTable {
    pub fn new<F>(evaluate: F, min_x: f32, max_x: f32, precision_scale: f32) -> Self
    where
        F: Fn(f32) -> f32,
    {
        let range = max_x - min_x;
        let n_values = (range * precision_scale) as usize + 1;

        let mut values = Vec::with_capacity(n_values);
        for i in 0..n_values {
            let x = i as f32 / precision_scale + min_x;
            let value = evaluate(x);
            values.push(value);
        }

        Self {
            values,
            precision_scale,
            min_x,
        }
    }

    #[inline]
    pub fn evaluate(&self, x: f32) -> f32 {
        let i = ((x - self.min_x) * self.precision_scale) as usize;
        self.values[i]
    }
}
