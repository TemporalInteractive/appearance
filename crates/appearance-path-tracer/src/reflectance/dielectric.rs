use glam::{Vec2, Vec3, Vec4};

use crate::{
    math::{
        normal::Normal,
        spherical_geometry::{abs_cos_theta, cos_theta, same_hemisphere},
        sqr,
    },
    radiometry::SampledSpectrum,
    reflectance::{reflect, refract},
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
    fn f(&self, wo: Vec3, wi: Vec3, transport_mode: TransportMode) -> SampledSpectrum {
        if self.eta == 1.0 || self.microfacet.is_smooth() {
            SampledSpectrum::new(Vec4::ZERO)
        } else {
            let cos_theta_o = cos_theta(wo);
            let cos_theta_i = cos_theta(wi);
            let reflect = cos_theta_i * cos_theta_o > 0.0;
            let mut etap = 1.0;
            if !reflect {
                etap = if cos_theta_o > 0.0 {
                    self.eta
                } else {
                    1.0 / self.eta
                };
            }

            let wm = wi * etap + wo;

            if cos_theta_i == 0.0 || cos_theta_o == 0.0 || wm.length_squared() == 0.0 {
                SampledSpectrum::new(Vec4::ZERO)
            } else {
                let wm = Normal(wm.normalize())
                    .forward_facing(&Vec3::new(0.0, 0.0, 1.0))
                    .0;
                if wm.dot(wi) * cos_theta_i < 0.0 || wm.dot(wo) * cos_theta_o < 0.0 {
                    SampledSpectrum::new(Vec4::ZERO)
                } else {
                    let f = fresnel_dielectric(wo.dot(wm), self.eta);

                    if reflect {
                        SampledSpectrum(Vec4::splat(
                            self.microfacet.d(wm) * self.microfacet.g(wo, wi) * f
                                / (4.0 * cos_theta_i * cos_theta_o).abs(),
                        ))
                    } else {
                        let denom = sqr(wi.dot(wm) + wo.dot(wm) / etap) * cos_theta_i * cos_theta_o;
                        let mut ft = self.microfacet.d(wm)
                            * (1.0 - f)
                            * self.microfacet.g(wo, wi)
                            * (wi.dot(wm) * wo.dot(wm) / denom).abs();

                        if transport_mode == TransportMode::Radiance {
                            ft /= sqr(etap);
                        }

                        SampledSpectrum(Vec4::splat(ft))
                    }
                }
            }
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
            let wm = self.microfacet.sample_wm(wo, u);
            let r = fresnel_dielectric(wo.dot(wm), self.eta);
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
                let wi = reflect(wo, wm);
                if !same_hemisphere(wo, wi) {
                    None
                } else {
                    let pdf =
                        self.microfacet.pdf(wo, wm) / (4.0 * wo.dot(wm).abs()) * pr / (pr + pt);
                    let f = self.microfacet.d(wm) * self.microfacet.g(wo, wi) * r
                        / (4.0 * cos_theta(wi) * cos_theta(wo));

                    Some(BsdfSample {
                        f: SampledSpectrum(Vec4::splat(f)),
                        wi,
                        pdf,
                        flags: BxdfFlags::GLOSSY_REFLECTION,
                        eta: 1.0,
                        pdf_is_proportional: false,
                    })
                }
            } else if let Some(refract_result) = refract(wo, Normal(wm), self.eta) {
                let wi = refract_result.wt;
                if same_hemisphere(wo, wi) || wi.z == 0.0 {
                    None
                } else {
                    let denom = sqr(wi.dot(wm) + wo.dot(wm) / refract_result.eta);
                    let dwm_dwi = wi.dot(wm).abs() / denom;
                    let pdf = self.microfacet.pdf(wo, wm) * dwm_dwi * pt / (pr + pt);

                    let mut ft = t
                        * self.microfacet.d(wm)
                        * self.microfacet.g(wo, wi)
                        * (wi.dot(wm) * wo.dot(wm) / (cos_theta(wi) * cos_theta(wo) * denom)).abs();
                    if transport_mode == TransportMode::Radiance {
                        ft /= sqr(refract_result.eta);
                    }

                    Some(BsdfSample {
                        f: SampledSpectrum(Vec4::splat(ft)),
                        wi,
                        pdf,
                        flags: BxdfFlags::GLOSSY_TRANSMISSION,
                        eta: refract_result.eta,
                        pdf_is_proportional: false,
                    })
                }
            } else {
                None
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
        if self.eta == 1.0 || self.microfacet.is_smooth() {
            0.0
        } else {
            let cos_theta_o = cos_theta(wo);
            let cos_theta_i = cos_theta(wi);
            let reflect = cos_theta_i * cos_theta_o > 0.0;
            let mut etap = 1.0;
            if !reflect {
                etap = if cos_theta_o > 0.0 {
                    self.eta
                } else {
                    1.0 / self.eta
                };
            }

            let wm = wi * etap + wo;

            if cos_theta_i == 0.0 || cos_theta_o == 0.0 || wm.length_squared() == 0.0 {
                0.0
            } else {
                let wm = Normal(wm.normalize())
                    .forward_facing(&Vec3::new(0.0, 0.0, 1.0))
                    .0;
                if wm.dot(wi) * cos_theta_i < 0.0 || wm.dot(wo) * cos_theta_o < 0.0 {
                    0.0
                } else {
                    let r = fresnel_dielectric(wo.dot(wm), self.eta);
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
                        0.0
                    } else if reflect {
                        self.microfacet.pdf(wo, wm) / (4.0 * wo.dot(wm).abs()) * pr / (pr + pt)
                    } else {
                        let denom = sqr(wi.dot(wm) + wo.dot(wm) / etap);
                        let dwm_dwi = wi.dot(wm).abs() / denom;
                        self.microfacet.pdf(wo, wm) * dwm_dwi * pt / (pr + pt)
                    }
                }
            }
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
