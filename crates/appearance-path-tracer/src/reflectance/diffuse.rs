use core::f32::consts::{FRAC_1_PI, PI};

use glam::{Vec2, Vec3, Vec4};

use crate::{
    math::spherical_geometry::{abs_cos_theta, same_hemisphere},
    radiometry::SampledSpectrum,
    sampling::{cosine_hemisphere_pdf, sample_cosine_hemisphere},
};

use super::{BsdfSample, Bxdf, BxdfFlags, BxdfReflTransFlags, TransportMode};

#[derive(Debug)]
pub struct DiffuseBxdf {
    reflectance: SampledSpectrum,
}

impl DiffuseBxdf {
    pub fn new(reflectance: SampledSpectrum) -> Self {
        Self { reflectance }
    }
}

impl Bxdf for DiffuseBxdf {
    fn f(&self, wo: Vec3, wi: Vec3, _transport_mode: TransportMode) -> SampledSpectrum {
        if !same_hemisphere(wo, wi) {
            SampledSpectrum::new(Vec4::ZERO)
        } else {
            SampledSpectrum::new(self.reflectance.0 * (1.0 / PI))
        }
    }

    fn sample_f(
        &self,
        wo: Vec3,
        _uc: f32,
        u: Vec2,
        _transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> Option<BsdfSample> {
        if !sample_flags.contains(BxdfReflTransFlags::REFLECTION) {
            None
        } else {
            let mut wi = sample_cosine_hemisphere(u);
            if wo.z < 0.0 {
                wi.z *= -1.0;
            }

            let pdf = cosine_hemisphere_pdf(abs_cos_theta(wi));

            let f = SampledSpectrum::new(self.reflectance.0 * FRAC_1_PI);

            Some(BsdfSample {
                f,
                wi,
                pdf,
                flags: self.flags(),
                eta: 1.0,
                pdf_is_proportional: false,
            })
        }
    }

    fn pdf(
        &self,
        wo: Vec3,
        wi: Vec3,
        _transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> f32 {
        if !sample_flags.contains(BxdfReflTransFlags::REFLECTION) || !same_hemisphere(wo, wi) {
            0.0
        } else {
            cosine_hemisphere_pdf(abs_cos_theta(wi))
        }
    }

    fn flags(&self) -> BxdfFlags {
        BxdfFlags::DIFFUSE_REFLECTION
    }
}
