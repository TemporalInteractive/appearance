use core::f32::consts::PI;

use glam::{Vec2, Vec3};

use crate::{
    math::{interaction::Interaction, sqr},
    radiometry::{DenselySampledSpectrum, SampledSpectrum, SampledWavelengths, Spectrum},
};

use super::{LightSource, LightSourceLiSample, LightSourceSampleCtx, LightSourceType};

pub struct DistantLight {
    direction: Vec3,
    intensity: DenselySampledSpectrum,
    scale: f32,
    scene_radius: f32,
}

impl DistantLight {
    pub fn new(
        direction: Vec3,
        intensity: DenselySampledSpectrum,
        scale: f32,
        scene_radius: f32,
    ) -> Self {
        Self {
            direction,
            intensity,
            scale,
            scene_radius,
        }
    }
}

impl LightSource for DistantLight {
    fn phi(&self, wavelengths: &SampledWavelengths) -> SampledSpectrum {
        SampledSpectrum::new(
            self.scale * self.intensity.sample(wavelengths).0 * PI * sqr(self.scene_radius),
        )
    }

    fn ty(&self) -> LightSourceType {
        LightSourceType::DeltaDirection
    }

    fn sample_li(
        &self,
        ctx: LightSourceSampleCtx,
        _u: Vec2,
        wavelengths: &SampledWavelengths,
        _allow_incomplete_pdf: bool,
    ) -> Option<LightSourceLiSample> {
        let wi = -self.direction;
        let p = ctx.point + wi * (2.0 * self.scene_radius);

        let li = SampledSpectrum::new(self.scale * self.intensity.sample(wavelengths).0);

        Some(LightSourceLiSample {
            l: li,
            wi,
            pdf: 1.0,
            light_interaction: Interaction::new_from_point(p),
        })
    }

    fn pdf_li(&self, _ctx: LightSourceSampleCtx, _wi: Vec3, _allow_incomplete_pdf: bool) -> f32 {
        0.0
    }
}
