use core::marker::{Send, Sync};

use glam::{Vec2, Vec3, Vec4};
use tinybvh::Ray;

use crate::{
    math::{
        interaction::{Interaction, SurfaceInteraction},
        normal::Normal,
    },
    radiometry::{SampledSpectrum, SampledWavelengths},
};

pub mod distant_light;
pub mod infinite_light;
pub mod point_light;

pub mod uniform_light_sampler;

pub enum LightSourceType {
    DeltaPosition,
    DeltaDirection,
    Area,
    Infinite,
}

impl LightSourceType {
    pub fn is_delta(&self) -> bool {
        matches!(self, Self::DeltaDirection | Self::DeltaPosition)
    }
}

#[derive(Debug, Clone, Copy, Default)]
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

    pub fn offset_ray_origin(&mut self, pt: Vec3) {
        self.point += (pt - self.point) * 0.00001; // TODO: ??
    }
}

pub struct LightSourceLiSample {
    pub l: SampledSpectrum,
    pub wi: Vec3,
    pub pdf: f32,
    pub light_interaction: Interaction,
}

pub trait LightSource: Send + Sync {
    fn phi(&self, wavelengths: &SampledWavelengths) -> SampledSpectrum;
    fn ty(&self) -> LightSourceType;
    fn sample_li(
        &self,
        ctx: LightSourceSampleCtx,
        u: Vec2,
        wavelengths: &SampledWavelengths,
        allow_incomplete_pdf: bool,
    ) -> Option<LightSourceLiSample>;
    fn pdf_li(&self, ctx: LightSourceSampleCtx, wi: Vec3, allow_incomplete_pdf: bool) -> f32;

    fn l(
        &self,
        _p: Vec3,
        _n: Normal,
        _uv: Vec2,
        _w: Vec3,
        _wavelengths: &SampledWavelengths,
    ) -> SampledSpectrum {
        SampledSpectrum(Vec4::ZERO)
    }

    fn le(&self, _ray: &Ray, _wavelengths: &SampledWavelengths) -> SampledSpectrum {
        SampledSpectrum(Vec4::ZERO)
    }
}

pub struct SampledLightSource<'a> {
    pub light_source: &'a dyn LightSource,
    pub pdf: f32,
}

pub trait LightSourceSampler: Send + Sync {
    fn sample_with_ctx(&self, ctx: LightSourceSampleCtx, u: f32) -> Option<SampledLightSource>;
    fn sample(&self, u: f32) -> Option<SampledLightSource>;
    fn pmf_with_ctx(&self, ctx: LightSourceSampleCtx, light_source: &dyn LightSource) -> f32;
    fn pmf(&self, light_source: &dyn LightSource) -> f32;
}
