use bitflags::bitflags;
use std::fmt::Debug;

use glam::{Vec2, Vec3, Vec4};

use crate::{
    math::spherical_geometry::abs_cos_theta, radiometry::SampledSpectrum,
    sampling::sample_uniform_hemisphere,
};

pub enum TransportMode {
    Radiance,
}

pub struct BsdfSample {
    f: SampledSpectrum,
    wi: Vec3,
    pdf: f32,
    flags: BxdfFlags,
    eta: f32,
    pdf_is_proportional: bool,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BxdfFlags: u32 {
        const REFLECTION = 0b00000001;
        const TRANSMISSION = 0b00000010;
        const DIFFUSE = 0b00001000;
        const GLOSSY = 0b00010000;
        const SPECULAR = 0b00100000;

        const DIFFUSE_REFLECTION = Self::DIFFUSE.bits() | Self::REFLECTION.bits();
        const DIFFUSE_TRANSMISSION = Self::DIFFUSE.bits() | Self::TRANSMISSION.bits();
        const GLOSSY_REFLECTION = Self::GLOSSY.bits() | Self::REFLECTION.bits();
        const GLOSSY_TRANSMISSION = Self::GLOSSY.bits() | Self::TRANSMISSION.bits();
        const SPECULAR_REFLECTION = Self::SPECULAR.bits() | Self::REFLECTION.bits();
        const SPECULAR_TRANSMISSION = Self::SPECULAR.bits() | Self::TRANSMISSION.bits();
    }
}

impl Default for BxdfFlags {
    fn default() -> Self {
        BxdfFlags::all()
    }
}

pub trait Bxdf: Debug {
    fn f(&self, wo: Vec3, wi: Vec3, transport_mode: TransportMode) -> SampledSpectrum;
    fn sample_f(
        &self,
        wo: Vec3,
        uc: f32,
        u: Vec2,
        transport_mode: TransportMode,
    ) -> Option<BsdfSample>;
    fn pdf(
        &self,
        wo: Vec3,
        wi: Vec3,
        transport_mode: TransportMode,
        sample_flags: BxdfFlags,
    ) -> f32;

    fn rho_from_wo(&self, wo: Vec3, uc: &[f32], u2: &[Vec2]) -> SampledSpectrum {
        let mut sampled_spectrum = Vec4::ZERO;

        for i in 0..uc.len() {
            let bs = self.sample_f(wo, uc[i], u2[i], TransportMode::Radiance);
            if let Some(bs) = bs {
                sampled_spectrum += bs.f.0 * abs_cos_theta(bs.wi) / bs.pdf;
            }
        }

        SampledSpectrum::new(sampled_spectrum)
    }

    fn rho(&self, u1: &[Vec2], uc: &[f32], u2: &[Vec2]) -> SampledSpectrum {
        let mut sampled_spectrum = Vec4::ZERO;

        for i in 0..uc.len() {
            let wo = sample_uniform_hemisphere(u1[i]);

            let bs = self.sample_f(wo, uc[i], u2[i], TransportMode::Radiance);
            if let Some(bs) = bs {
                sampled_spectrum += bs.f.0 * abs_cos_theta(bs.wi) / bs.pdf;
            }
        }

        SampledSpectrum::new(sampled_spectrum)
    }
}
