use glam::{Vec2, Vec3, Vec4};

use crate::{
    math::{
        normal::Normal,
        spherical_geometry::{abs_cos_theta, same_hemisphere},
    },
    radiometry::SampledSpectrum,
    reflectance::reflect,
};

use super::{
    fresnel_complex_spectrum, microfacet::ThrowbridgeReitzDistribution, BsdfSample, Bxdf,
    BxdfFlags, BxdfReflTransFlags, TransportMode,
};

#[derive(Debug)]
pub struct ConductorBxdf {
    microfacet: ThrowbridgeReitzDistribution,
    eta: SampledSpectrum,
    k: SampledSpectrum,
}

impl ConductorBxdf {
    pub fn new(
        microfacet: ThrowbridgeReitzDistribution,
        eta: SampledSpectrum,
        k: SampledSpectrum,
    ) -> Self {
        Self { microfacet, eta, k }
    }
}

impl Bxdf for ConductorBxdf {
    fn f(&self, wo: Vec3, wi: Vec3, _transport_mode: TransportMode) -> SampledSpectrum {
        if !same_hemisphere(wo, wi) || self.microfacet.is_smooth() {
            SampledSpectrum::new(Vec4::ZERO)
        } else {
            let cos_theta_o = abs_cos_theta(wo);
            let cos_theta_i = abs_cos_theta(wi);

            if cos_theta_i == 0.0 || cos_theta_o == 0.0 {
                SampledSpectrum::new(Vec4::ZERO)
            } else {
                let mut wm = wi + wo;
                if wm.length_squared() == 0.0 {
                    SampledSpectrum::new(Vec4::ZERO)
                } else {
                    wm = wm.normalize();
                    let f = fresnel_complex_spectrum(wo.dot(wm).abs(), self.eta, self.k);

                    SampledSpectrum::new(
                        self.microfacet.d(wm) * f.0 * self.microfacet.g(wo, wi)
                            / (4.0 * cos_theta_i * cos_theta_o),
                    )
                }
            }
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
        } else if self.microfacet.is_smooth() {
            let wi = Vec3::new(-wo.x, -wo.y, wo.z);
            let f = SampledSpectrum::new(
                fresnel_complex_spectrum(abs_cos_theta(wi), self.eta, self.k).0 / abs_cos_theta(wi),
            );

            println!("{:?}", f);

            Some(BsdfSample {
                f,
                wi,
                pdf: 1.0,
                flags: self.flags(),
                eta: 1.0,
                pdf_is_proportional: false,
            })
        } else {
            let wm = self.microfacet.sample_wm(wo, u);
            let wi = reflect(wo, wm);
            if !same_hemisphere(wo, wi) {
                None
            } else {
                let wo_dot_wm = wo.dot(wm).abs();

                let pdf = self.microfacet.pdf(wo, wm) / (4.0 * wo_dot_wm);

                let cos_theta_o = abs_cos_theta(wo);
                let cos_theta_i = abs_cos_theta(wi);

                let f = fresnel_complex_spectrum(wo_dot_wm, self.eta, self.k);
                let f = self.microfacet.d(wm) * f.0 * self.microfacet.g(wo, wi)
                    / (4.0 * cos_theta_i * cos_theta_o);

                Some(BsdfSample {
                    f: SampledSpectrum(f),
                    wi,
                    pdf,
                    flags: self.flags(),
                    eta: 1.0,
                    pdf_is_proportional: false,
                })
            }
        }
    }

    fn pdf(
        &self,
        wo: Vec3,
        wi: Vec3,
        _transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> f32 {
        if !sample_flags.contains(BxdfReflTransFlags::REFLECTION)
            || !same_hemisphere(wo, wi)
            || self.microfacet.is_smooth()
        {
            0.0
        } else {
            let mut wm = wo + wi;
            if wm.length_squared() == 0.0 {
                0.0
            } else {
                wm = Normal(wm.normalize())
                    .forward_facing(&Vec3::new(0.0, 0.0, 1.0))
                    .0;

                self.microfacet.pdf(wo, wm) / (4.0 * wo.dot(wm).abs())
            }
        }
    }

    fn flags(&self) -> BxdfFlags {
        if self.microfacet.is_smooth() {
            BxdfFlags::SPECULAR_REFLECTION
        } else {
            BxdfFlags::GLOSSY_REFLECTION
        }
    }
}
