use glam::{Vec2, Vec3};

use crate::{
    math::{
        interaction::{Interaction, SurfaceInteraction},
        normal::Normal,
    },
    radiometry::{SampledSpectrum, SampledWavelengths},
};

pub mod point_light;

pub enum LightSourceType {
    DeltaPosition,
    DeltaDirection,
    Area,
    Infinite,
}

pub struct LightSourceSampleCtx {
    pub point: Vec3,
    pub normal: Normal,
    pub shading_normal: Normal,
}

impl LightSourceSampleCtx {
    pub fn new_from_surface(surface_interaction: SurfaceInteraction) -> Self {
        Self {
            point: surface_interaction.interaction.point,
            normal: surface_interaction.interaction.normal,
            shading_normal: surface_interaction.shading_normal,
        }
    }

    pub fn new_from_medium(interaction: Interaction) -> Self {
        Self {
            point: interaction.point,
            normal: Normal::new(Vec3::ZERO),
            shading_normal: Normal::new(Vec3::ZERO),
        }
    }
}

pub struct LightSourceLiSample {
    pub l: SampledSpectrum,
    pub wi: Vec3,
    pub pdf: f32,
    pub light_interaction: Interaction,
}

pub trait LightSource {
    fn phi(&self, wavelengths: SampledWavelengths) -> SampledSpectrum;
    fn ty(&self) -> LightSourceType;
    fn sample_li(
        &self,
        ctx: LightSourceSampleCtx,
        u: Vec2,
        wavelengths: SampledWavelengths,
        allow_incomplete_pdf: f32,
    ) -> Option<LightSourceLiSample>;
    fn pdf_li(&self, ctx: LightSourceSampleCtx, wi: Vec3, allow_incomplete_pdf: f32) -> f32;
}
