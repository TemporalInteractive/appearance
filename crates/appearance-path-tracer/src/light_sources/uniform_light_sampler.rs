use super::{LightSource, LightSourceSampleCtx, LightSourceSampler, SampledLightSource};

pub struct UniformLightSourceSampler {
    lights: Vec<Box<dyn LightSource>>,
}

impl UniformLightSourceSampler {
    pub fn new(lights: Vec<Box<dyn LightSource>>) -> Self {
        Self { lights }
    }
}

impl LightSourceSampler for UniformLightSourceSampler {
    fn sample_with_ctx(&self, _ctx: LightSourceSampleCtx, u: f32) -> Option<SampledLightSource> {
        self.sample(u)
    }

    fn sample(&self, u: f32) -> Option<SampledLightSource> {
        if self.lights.is_empty() {
            None
        } else {
            let idx = ((u * self.lights.len() as f32) as usize).min(self.lights.len() - 1);

            Some(SampledLightSource {
                light_source: self.lights[idx].as_ref(),
                pdf: 1.0 / self.lights.len() as f32,
            })
        }
    }

    fn pmf_with_ctx(&self, _ctx: LightSourceSampleCtx, light_source: &dyn LightSource) -> f32 {
        self.pmf(light_source)
    }

    fn pmf(&self, _light_source: &dyn LightSource) -> f32 {
        if self.lights.is_empty() {
            0.0
        } else {
            1.0 / self.lights.len() as f32
        }
    }
}
