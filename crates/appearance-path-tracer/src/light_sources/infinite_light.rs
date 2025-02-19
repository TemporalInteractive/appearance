use core::f32::consts::PI;

use glam::{Vec2, Vec3};

use crate::{
    math::interaction::Interaction,
    radiometry::{DenselySampledSpectrum, SampledSpectrum, SampledWavelengths, Spectrum},
};

use super::{LightSource, LightSourceLiSample, LightSourceSampleCtx, LightSourceType};

pub struct InfiniteLight {
    position: Vec3,
    intensity: DenselySampledSpectrum,
    scale: f32,
}

impl InfiniteLight {
    pub fn new(position: Vec3, intensity: DenselySampledSpectrum, scale: f32) -> Self {
        Self {
            position,
            intensity,
            scale,
        }
    }
}

impl LightSource for InfiniteLight {
    fn phi(&self, wavelengths: &SampledWavelengths) -> SampledSpectrum {
        SampledSpectrum::new(4.0 * PI * self.scale * self.intensity.sample(wavelengths).0)
    }

    fn ty(&self) -> LightSourceType {
        LightSourceType::DeltaPosition
    }

    fn sample_li(
        &self,
        ctx: LightSourceSampleCtx,
        _u: Vec2,
        wavelengths: &SampledWavelengths,
        _allow_incomplete_pdf: bool,
    ) -> Option<LightSourceLiSample> {
        let wi = (self.position - ctx.point).normalize();
        let li = SampledSpectrum::new(
            self.scale * self.intensity.sample(wavelengths).0
                / self.position.distance_squared(ctx.point),
        );

        Some(LightSourceLiSample {
            l: li,
            wi,
            pdf: 1.0,
            light_interaction: Interaction::new_from_point(self.position),
        })
    }

    fn pdf_li(&self, _ctx: LightSourceSampleCtx, _wi: Vec3, _allow_incomplete_pdf: bool) -> f32 {
        0.0
    }
}
