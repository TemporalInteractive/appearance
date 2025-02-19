use core::f32::consts::PI;
use std::sync::Arc;

use appearance_texture::{Texture, TextureSampleInterpolation, TextureSampleRepeat};
use glam::{UVec2, Vec2, Vec3, Vec4, Vec4Swizzles};
use tinybvh::Ray;

use crate::{
    math::{interaction::Interaction, sqr},
    radiometry::{
        Rgb, RgbColorSpace, RgbIlluminantSpectrum, SampledSpectrum, SampledWavelengths, Spectrum,
    },
    sampling::{
        panorama_coords_to_unit_vector, piecewise_constant::PiecewiseConstant2D,
        unit_vector_to_panorama_coords,
    },
};

use super::{LightSource, LightSourceLiSample, LightSourceSampleCtx, LightSourceType};

pub struct InfiniteLight {
    texture: Arc<Texture>,
    color_space: Arc<RgbColorSpace>,
    scale: f32,
    scene_radius: f32,
    distribution: PiecewiseConstant2D,
    compensated_distribution: PiecewiseConstant2D,
}

impl InfiniteLight {
    pub fn new(
        texture: Arc<Texture>,
        color_space: Arc<RgbColorSpace>,
        scale: f32,
        scene_radius: f32,
    ) -> Self {
        let texture_distrbution = texture.get_sampling_distribution();
        let distribution =
            PiecewiseConstant2D::new_from_2d(texture_distrbution.clone(), [0.0; 2], [1.0; 2]);

        let mut componsated_texture_distrbution = texture_distrbution;
        let mut average = 0.0;
        for row in &mut componsated_texture_distrbution {
            for v in row {
                average += *v;
            }
        }
        average /= (componsated_texture_distrbution.len()
            * componsated_texture_distrbution[0].len()) as f32;
        for row in &mut componsated_texture_distrbution {
            for v in row {
                *v = (*v - average).max(0.0);
            }
        }

        let compensated_distribution =
            PiecewiseConstant2D::new_from_2d(componsated_texture_distrbution, [0.0; 2], [1.0; 2]);

        Self {
            texture,
            color_space,
            scale,
            scene_radius,
            distribution,
            compensated_distribution,
        }
    }

    fn image_le(&self, uv: Vec2, wavelengths: &SampledWavelengths) -> SampledSpectrum {
        let rgb = Rgb(self
            .texture
            .sample(
                uv,
                TextureSampleRepeat::Clamp,
                TextureSampleInterpolation::Nearest,
            )
            .xyz());
        let spectrum = RgbIlluminantSpectrum::new(rgb, &self.color_space);
        SampledSpectrum(spectrum.sample(wavelengths).0 * self.scale)
    }
}

impl LightSource for InfiniteLight {
    fn phi(&self, wavelengths: &SampledWavelengths) -> SampledSpectrum {
        let mut sum_l = Vec4::ZERO;

        for y in 0..self.texture.height() {
            for x in 0..self.texture.width() {
                let rgb = Rgb(self.texture.load(UVec2::new(x, y)).xyz());
                sum_l += RgbIlluminantSpectrum::new(rgb, &self.color_space)
                    .sample(wavelengths)
                    .0;
            }
        }

        SampledSpectrum(
            4.0 * PI * PI * sqr(self.scene_radius) * sum_l
                / (self.texture.width() * self.texture.height()) as f32,
        )
    }

    fn ty(&self) -> LightSourceType {
        LightSourceType::Infinite
    }

    fn sample_li(
        &self,
        _ctx: LightSourceSampleCtx,
        u: Vec2,
        wavelengths: &SampledWavelengths,
        allow_incomplete_pdf: bool,
    ) -> Option<LightSourceLiSample> {
        let sample = if allow_incomplete_pdf {
            self.compensated_distribution.sample(u)
        } else {
            self.distribution.sample(u)
        };

        if sample.pdf == 0.0 {
            None
        } else {
            let wi = panorama_coords_to_unit_vector(sample.value);
            let pdf = sample.pdf / (4.0 * PI);

            Some(LightSourceLiSample {
                l: self.image_le(sample.value, wavelengths),
                wi,
                pdf,
                light_interaction: Interaction::new_from_point(Vec3::ZERO),
            })
        }
    }

    fn pdf_li(&self, _ctx: LightSourceSampleCtx, wi: Vec3, allow_incomplete_pdf: bool) -> f32 {
        let uv = unit_vector_to_panorama_coords(wi);
        let pdf = if allow_incomplete_pdf {
            self.compensated_distribution.pdf(uv)
        } else {
            self.distribution.pdf(uv)
        };

        pdf / (4.0 * PI)
    }

    fn le(&self, ray: &Ray, wavelengths: &SampledWavelengths) -> SampledSpectrum {
        let uv = unit_vector_to_panorama_coords(ray.D.into());
        if uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 {
            println!("{:?} {:?}", uv, Vec3::from(ray.D));
        }
        self.image_le(uv, wavelengths)
    }
}
