use appearance_texture::{TextureSampleInterpolation, TextureSampleRepeat};
use glam::{Vec3, Vec4, Vec4Swizzles};
use tinybvh::Ray;

use crate::{
    geometry_resources::GeometryResources,
    light_sources::{LightSource, LightSourceSampleCtx},
    math::{interaction::Interaction, normal::Normal},
    radiometry::{
        PiecewiseLinearSpectrum, Rgb, RgbAlbedoSpectrum, RgbColorSpace, SampledSpectrum,
        SampledWavelengths, Spectrum,
    },
    reflectance::{
        conductor::ConductorBxdf, diffuse::DiffuseBxdf, microfacet::ThrowbridgeReitzDistribution,
        Bsdf, BxdfReflTransFlags, TransportMode,
    },
    sampling::Sampler,
};

pub struct PathIntegrator {
    max_bounces: u32,
}

impl PathIntegrator {
    pub fn new(max_bounces: u32) -> Self {
        Self { max_bounces }
    }

    pub fn li(
        &self,
        mut ray: Ray,
        wavelengths: &SampledWavelengths,
        mut sampler: Box<dyn Sampler>,
        geometry_resources: &GeometryResources,
    ) -> SampledSpectrum {
        let mut l = Vec4::ZERO;
        let mut throughput = Vec4::ONE;

        let mut bounce = 0;
        loop {
            geometry_resources.tlas().intersect(&mut ray);

            let interaction =
                Interaction::new_from_point(Vec3::from(ray.O) + Vec3::from(ray.D) * ray.hit.t);
            let hit_data = geometry_resources.get_hit_data(&ray.hit);

            if ray.hit.t == 1e30 {
                let sky_l = geometry_resources.infinite_light.le(&ray, wavelengths);
                l += throughput * sky_l.0;
                break;
            }

            // let normal_f = RgbAlbedoSpectrum::new(
            //     Rgb(hit_data.normal * 0.5 + 0.5),
            //     RgbColorSpace::srgb().as_ref(),
            // )
            // .sample(wavelengths);
            // l += normal_f.0;
            // break;

            // TODO: triangle light radiance on hit

            bounce += 1;
            if bounce > self.max_bounces {
                break;
            }

            let mut base_color_factor = hit_data.material.base_color_factor.xyz();

            if let Some(tex_coord) = hit_data.tex_coord {
                if let Some(base_color_texture) = &hit_data.material.base_color_texture {
                    base_color_factor *= base_color_texture
                        .sample(
                            tex_coord,
                            TextureSampleRepeat::Repeat,
                            TextureSampleInterpolation::Linear,
                        )
                        .xyz();
                }
            }

            // TODO: get bsdf from intersection material
            let spectrum = RgbAlbedoSpectrum::new(Rgb(base_color_factor), &RgbColorSpace::srgb());
            let diffuse_bxdf = Box::new(DiffuseBxdf::new(spectrum.sample(wavelengths)));
            let bsdf = Bsdf::new(diffuse_bxdf, Normal(hit_data.normal), Vec3::ZERO);

            // let eta = PiecewiseLinearSpectrum::au_eta().sample(wavelengths);
            // let k = PiecewiseLinearSpectrum::au_k().sample(wavelengths);
            // let microfacet = ThrowbridgeReitzDistribution::new(0.1, 0.1);
            // let conductor_bxdf = Box::new(ConductorBxdf::new(microfacet, eta, k));
            // let bsdf = Bsdf::new(conductor_bxdf, Normal(hit_data.normal), Vec3::ZERO);

            let wo = -Vec3::from(ray.D);

            if let Some(light_source_sample) =
                geometry_resources.light_sampler.sample(sampler.get_1d())
            {
                if let Some(light_sample) = light_source_sample.light_source.sample_li(
                    LightSourceSampleCtx::new_from_medium(interaction.clone()),
                    sampler.get_2d(),
                    wavelengths,
                    false,
                ) {
                    if light_sample.pdf > 0.0 && light_sample.l.has_contribution() {
                        let wi = light_sample.wi;
                        let f = SampledSpectrum::new(
                            bsdf.f(wo, wi, TransportMode::Radiance).0
                                * wi.dot(hit_data.normal).abs(),
                        );

                        if f.has_contribution() {
                            let mut shadow_ray = Ray::new(interaction.point + wi * 0.0001, wi);
                            shadow_ray.hit.t = interaction
                                .point
                                .distance(light_sample.light_interaction.point);
                            if !geometry_resources.tlas().is_occluded(&shadow_ray) {
                                l += throughput * f.0 * light_sample.l.0
                                    / (light_source_sample.pdf * light_sample.pdf);
                                // TODO: dont forget the light sampler pdf in the future
                            }
                        }
                    }
                }
            }

            let uc = sampler.get_1d();
            let u = sampler.get_2d();
            if let Some(bsdf_sample) = bsdf.sample_f(
                wo,
                uc,
                u,
                TransportMode::Radiance,
                BxdfReflTransFlags::all(),
            ) {
                throughput *=
                    bsdf_sample.f.0 * bsdf_sample.wi.dot(hit_data.normal).abs() / bsdf_sample.pdf;
                ray = Ray::new(interaction.point + bsdf_sample.wi * 0.0001, bsdf_sample.wi);
            } else {
                break;
            }
        }

        SampledSpectrum::new(l)
    }
}
