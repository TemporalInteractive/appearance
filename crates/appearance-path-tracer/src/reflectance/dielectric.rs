use glam::{Vec2, Vec3, Vec4};

use crate::{
    math::{
        normal::Normal,
        spherical_geometry::{abs_cos_theta, cos_theta},
        sqr,
    },
    radiometry::SampledSpectrum,
    reflectance::refract,
};

use super::{
    fresnel_dielectric, microfacet::ThrowbridgeReitzDistribution, BsdfSample, Bxdf, BxdfFlags,
    BxdfReflTransFlags, TransportMode,
};

#[derive(Debug)]
pub struct DielectricBxdf {
    microfacet: ThrowbridgeReitzDistribution,
    eta: f32,
}

impl DielectricBxdf {
    pub fn new(microfacet: ThrowbridgeReitzDistribution, eta: f32) -> Self {
        Self { microfacet, eta }
    }
}

impl Bxdf for DielectricBxdf {
    fn f(&self, wo: Vec3, wi: Vec3, _transport_mode: TransportMode) -> SampledSpectrum {
        if self.eta == 1.0 || self.microfacet.is_smooth() {
            SampledSpectrum::new(Vec4::ZERO)
        } else {
            todo!()
        }
    }

    fn sample_f(
        &self,
        wo: Vec3,
        uc: f32,
        u: Vec2,
        transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> Option<BsdfSample> {
        if self.eta == 1.0 || self.microfacet.is_smooth() {
            let r = fresnel_dielectric(cos_theta(wo), self.eta);
            let t = 1.0 - r;

            let pr = if !sample_flags.contains(BxdfReflTransFlags::REFLECTION) {
                0.0
            } else {
                r
            };
            let pt = if !sample_flags.contains(BxdfReflTransFlags::TRANSMISSION) {
                0.0
            } else {
                t
            };

            if pr == 0.0 && pt == 0.0 {
                None
            } else if uc < pr / (pr + pt) {
                let wi = Vec3::new(-wo.x, -wo.y, wo.z);
                let fr = SampledSpectrum(Vec4::splat(r / abs_cos_theta(wi)));

                Some(BsdfSample {
                    f: fr,
                    wi,
                    pdf: pr / (pr + pt),
                    flags: BxdfFlags::SPECULAR_REFLECTION,
                    eta: 1.0,
                    pdf_is_proportional: false,
                })
            } else if let Some(refract_result) =
                refract(wo, Normal(Vec3::new(0.0, 0.0, 1.0)), self.eta)
            {
                let mut ft = Vec4::splat(t / abs_cos_theta(refract_result.wt));
                if transport_mode == TransportMode::Radiance {
                    ft /= sqr(refract_result.eta);
                }

                Some(BsdfSample {
                    f: SampledSpectrum(ft),
                    wi: refract_result.wt,
                    pdf: pt / (pr + pt),
                    flags: BxdfFlags::SPECULAR_TRANSMISSION,
                    eta: refract_result.eta,
                    pdf_is_proportional: false,
                })
            } else {
                None
            }
        } else {
            todo!()
        }
    }

    fn pdf(
        &self,
        wo: Vec3,
        wi: Vec3,
        _transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> f32 {
        if self.eta == 1.0 || self.microfacet.is_smooth() {
            0.0
        } else {
            todo!()
        }
    }

    fn flags(&self) -> BxdfFlags {
        let mut flags = if self.eta == 1.0 {
            BxdfFlags::TRANSMISSION
        } else {
            BxdfFlags::TRANSMISSION | BxdfFlags::REFLECTION
        };

        flags |= if self.microfacet.is_smooth() {
            BxdfFlags::SPECULAR
        } else {
            BxdfFlags::GLOSSY
        };

        flags
    }
}
