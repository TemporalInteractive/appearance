use bitflags::bitflags;
use std::fmt::Debug;

use glam::{Vec2, Vec3, Vec4};

use crate::{
    math::{coord_system::CoordSystem, normal::Normal, spherical_geometry::abs_cos_theta},
    radiometry::SampledSpectrum,
    sampling::sample_uniform_hemisphere,
};

pub mod diffuse;
pub enum TransportMode {
    Radiance,
}
pub struct BsdfSample {
    pub f: SampledSpectrum,
    pub wi: Vec3,
    pub pdf: f32,
    pub flags: BxdfFlags,
    pub eta: f32,
    pub pdf_is_proportional: bool,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct BxdfReflTransFlags: u32 {
        const REFLECTION = 0b00000001;
        const TRANSMISSION = 0b00000010;
    }
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

impl From<BxdfReflTransFlags> for BxdfFlags {
    fn from(value: BxdfReflTransFlags) -> Self {
        let mut flags = BxdfFlags::empty();
        flags.set(
            BxdfFlags::REFLECTION,
            value.contains(BxdfReflTransFlags::REFLECTION),
        );
        flags.set(
            BxdfFlags::TRANSMISSION,
            value.contains(BxdfReflTransFlags::TRANSMISSION),
        );
        flags
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
        sample_flags: BxdfReflTransFlags,
    ) -> Option<BsdfSample>;
    fn pdf(
        &self,
        wo: Vec3,
        wi: Vec3,
        transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> f32;

    fn flags(&self) -> BxdfFlags;

    fn rho_from_wo(&self, wo: Vec3, uc: &[f32], u2: &[Vec2]) -> SampledSpectrum {
        let mut sampled_spectrum = Vec4::ZERO;

        for i in 0..uc.len() {
            let bs = self.sample_f(
                wo,
                uc[i],
                u2[i],
                TransportMode::Radiance,
                BxdfReflTransFlags::all(),
            );
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

            let bs = self.sample_f(
                wo,
                uc[i],
                u2[i],
                TransportMode::Radiance,
                BxdfReflTransFlags::all(),
            );
            if let Some(bs) = bs {
                sampled_spectrum += bs.f.0 * abs_cos_theta(bs.wi) / bs.pdf;
            }
        }

        SampledSpectrum::new(sampled_spectrum)
    }
}

pub struct Bsdf {
    bxdf: Box<dyn Bxdf>,
    shading_cs: CoordSystem,
}

impl Bsdf {
    pub fn new(bxdf: Box<dyn Bxdf>, shading_normal: Normal, shading_dpdu: Vec3) -> Self {
        //let shading_cs = CoordSystem::from_xz(shading_dpdu.normalize(), shading_normal.0); // TODO
        let shading_cs = CoordSystem::from_z(shading_normal.0);

        Self { bxdf, shading_cs }
    }

    pub fn flags(&self) -> BxdfFlags {
        self.bxdf.flags()
    }

    pub fn render_to_local(&self, v: Vec3) -> Vec3 {
        self.shading_cs.ws_to_frame(v)
    }

    pub fn local_to_render(&self, v: Vec3) -> Vec3 {
        self.shading_cs.frame_to_ws(v)
    }

    pub fn f(
        &self,
        wo_render: Vec3,
        wi_render: Vec3,
        transport_mode: TransportMode,
    ) -> SampledSpectrum {
        let wi = self.render_to_local(wi_render);
        let wo = self.render_to_local(wo_render);

        if wo.z == 0.0 {
            SampledSpectrum::new(Vec4::ZERO)
        } else {
            self.bxdf.f(wo, wi, transport_mode)
        }
    }

    pub fn sample_f(
        &self,
        wo_render: Vec3,
        u: f32,
        u2: Vec2,
        transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> Option<BsdfSample> {
        let wo = self.render_to_local(wo_render);
        if wo.z == 0.0 || self.bxdf.flags().contains(BxdfFlags::from(sample_flags)) {
            None
        } else {
            let mut bs = self.bxdf.sample_f(wo, u, u2, transport_mode, sample_flags);
            if let Some(bs) = &mut bs {
                if !bs.f.has_contribution() || bs.pdf == 0.0 || bs.wi.z == 0.0 {
                    return None;
                }

                bs.wi = self.local_to_render(bs.wi);
            }

            bs
        }
    }

    pub fn pdf(
        &self,
        wo_render: Vec3,
        wi_render: Vec3,
        transport_mode: TransportMode,
        sample_flags: BxdfReflTransFlags,
    ) -> f32 {
        let wi = self.render_to_local(wi_render);
        let wo = self.render_to_local(wo_render);

        if wo.z == 0.0 {
            0.0
        } else {
            self.bxdf.pdf(wo, wi, transport_mode, sample_flags)
        }
    }
}
