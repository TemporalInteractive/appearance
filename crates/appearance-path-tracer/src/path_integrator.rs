use appearance_texture::{TextureSampleInterpolation, TextureSampleRepeat};
use glam::{Vec3, Vec4, Vec4Swizzles};
use tinybvh::Ray;

use crate::{
    geometry_resources::GeometryResources,
    light_sources::{LightSource, LightSourceSampleCtx},
    math::{
        interaction::{Interaction, SurfaceInteraction},
        normal::Normal,
        sqr,
    },
    radiometry::{
        PiecewiseLinearSpectrum, Rgb, RgbAlbedoSpectrum, RgbColorSpace, SampledSpectrum,
        SampledWavelengths, Spectrum,
    },
    reflectance::{
        conductor::ConductorBxdf, dielectric::DielectricBxdf, diffuse::DiffuseBxdf,
        microfacet::ThrowbridgeReitzDistribution, Bsdf, Bxdf, BxdfFlags, BxdfReflTransFlags,
        TransportMode,
    },
    sampling::{power_heuristic, Sampler},
};

pub struct PathIntegrator {
    max_bounces: u32,
}

impl PathIntegrator {
    pub fn new(max_bounces: u32) -> Self {
        Self { max_bounces }
    }

    pub fn sample_ld(
        surface_interaction: SurfaceInteraction,
        bsdf: &Bsdf,
        wavelengths: &SampledWavelengths,
        sampler: &mut Box<dyn Sampler>,
        geometry_resources: &GeometryResources,
    ) -> SampledSpectrum {
        let mut light_sample_ctx =
            LightSourceSampleCtx::new_from_surface(surface_interaction.clone());

        if bsdf.flags().contains(BxdfFlags::REFLECTION)
            && !bsdf.flags().contains(BxdfFlags::TRANSMISSION)
        {
            light_sample_ctx.offset_ray_origin(surface_interaction.interaction.wo);
        } else if bsdf.flags().contains(BxdfFlags::TRANSMISSION)
            && !bsdf.flags().contains(BxdfFlags::REFLECTION)
        {
            light_sample_ctx.offset_ray_origin(-surface_interaction.interaction.wo);
        }

        let u_light = sampler.get_2d();
        if let Some(light_source_sample) = geometry_resources.light_sampler.sample(sampler.get_1d())
        {
            if let Some(light_sample) = light_source_sample.light_source.sample_li(
                light_sample_ctx,
                u_light,
                wavelengths,
                true,
            ) {
                if light_sample.pdf > 0.0 && light_sample.l.has_contribution() {
                    let wo = surface_interaction.interaction.wo;
                    let wi = light_sample.wi;
                    let f = SampledSpectrum::new(
                        bsdf.f(wo, wi, TransportMode::Radiance).0
                            * wi.dot(surface_interaction.shading_normal.0).abs(),
                    );

                    if f.has_contribution() {
                        let mut shadow_ray =
                            Ray::new(surface_interaction.interaction.point + wi * 0.0001, wi);
                        shadow_ray.hit.t = surface_interaction
                            .interaction
                            .point
                            .distance(light_sample.light_interaction.point);

                        if !geometry_resources.tlas().is_occluded(&shadow_ray) {
                            let p_l = light_source_sample.pdf * light_sample.pdf;

                            if light_source_sample.light_source.ty().is_delta() {
                                return SampledSpectrum(light_sample.l.0 * f.0 / p_l);
                            } else {
                                let p_b = bsdf.pdf(
                                    wo,
                                    wi,
                                    TransportMode::Radiance,
                                    BxdfReflTransFlags::all(),
                                );
                                let w_l = power_heuristic(1, p_l, 1, p_b);
                                return SampledSpectrum(w_l * light_sample.l.0 * f.0 / p_l);
                            };
                        }
                    }
                }
            }
        }

        SampledSpectrum(Vec4::ZERO)
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
        let mut p_b = 1.0;
        let mut eta_scale = 1.0;
        let mut specular_bounce = false;
        let mut any_non_specular_bounces = false;
        let mut prev_light_ctx = LightSourceSampleCtx::default();

        let mut depth = 0;
        loop {
            geometry_resources.tlas().intersect(&mut ray);

            let hit_point = Vec3::from(ray.O) + Vec3::from(ray.D) * ray.hit.t;
            let hit_data = geometry_resources.get_hit_data(&ray.hit);

            let interaction = Interaction {
                point: hit_point,
                wo: -Vec3::from(ray.D),
                normal: Normal(hit_data.normal),
                uv: hit_data.tex_coord.unwrap_or_default(), // TODO: is this not supposed to be the bary coords?
            };
            let surface_interaction = SurfaceInteraction {
                interaction: interaction.clone(),
                dpdu: Vec3::ZERO, // TODO: derivates
                dpdv: Vec3::ZERO,
                dndu: Normal(Vec3::ZERO),
                dndv: Normal(Vec3::ZERO),
                shading_normal: interaction.normal, // TODO: optional normal mapping
            };

            if ray.hit.t == 1e30 {
                let light_source = &geometry_resources.infinite_light;
                let le = light_source.le(&ray, wavelengths);

                if depth == 0 || specular_bounce {
                    l += throughput * le.0;
                } else {
                    let p_l = geometry_resources
                        .light_sampler
                        .pmf_with_ctx(prev_light_ctx, light_source)
                        * light_source.pdf_li(prev_light_ctx, ray.D.into(), true);

                    let w_b = power_heuristic(1, p_b, 1, p_l);

                    l += throughput * w_b * le.0;
                }

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

            depth += 1;
            if depth > self.max_bounces {
                break;
            }

            let mut base_color_factor = hit_data.material.base_color_factor.xyz();
            let mut metallic_factor = hit_data.material.metallic_factor;

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

                if let Some(metallic_roughness_texture) =
                    &hit_data.material.metallic_roughness_texture
                {
                    let metallic_roughness = metallic_roughness_texture
                        .sample(
                            tex_coord,
                            TextureSampleRepeat::Repeat,
                            TextureSampleInterpolation::Linear,
                        )
                        .xyz();
                    metallic_factor *= metallic_roughness.z;
                }
            }

            let bsdf = if metallic_factor > 0.9 {
                let eta = PiecewiseLinearSpectrum::al_eta().sample(wavelengths);
                let k = PiecewiseLinearSpectrum::al_k().sample(wavelengths);
                let microfacet = ThrowbridgeReitzDistribution::new(0.01, 0.01);
                let conductor_bxdf = Box::new(ConductorBxdf::new(microfacet, eta, k));
                Bsdf::new(conductor_bxdf, Normal(hit_data.normal), Vec3::ZERO)
            } else if hit_data.material.transmission_factor > 0.0 {
                let microfacet = ThrowbridgeReitzDistribution::new(0.0, 0.0);
                let dielectric_bxdf = Box::new(DielectricBxdf::new(microfacet, 1.5));
                Bsdf::new(dielectric_bxdf, Normal(hit_data.normal), Vec3::ZERO)
            } else {
                let spectrum =
                    RgbAlbedoSpectrum::new(Rgb(base_color_factor), &RgbColorSpace::srgb());
                let diffuse_bxdf = Box::new(DiffuseBxdf::new(spectrum.sample(wavelengths)));
                Bsdf::new(diffuse_bxdf, Normal(hit_data.normal), Vec3::ZERO)
            };

            // TODO: get bsdf from intersection material
            // let spectrum = RgbAlbedoSpectrum::new(Rgb(base_color_factor), &RgbColorSpace::srgb());
            // let diffuse_bxdf = Box::new(DiffuseBxdf::new(spectrum.sample(wavelengths)));
            // let bsdf = Bsdf::new(diffuse_bxdf, Normal(hit_data.normal), Vec3::ZERO);

            // let eta = PiecewiseLinearSpectrum::au_eta().sample(wavelengths);
            // let k = PiecewiseLinearSpectrum::au_k().sample(wavelengths);
            // let microfacet = ThrowbridgeReitzDistribution::new(0.01, 0.01);
            // let conductor_bxdf = Box::new(ConductorBxdf::new(microfacet, eta, k));
            // let bsdf = Bsdf::new(conductor_bxdf, Normal(hit_data.normal), Vec3::ZERO);

            // let microfacet = ThrowbridgeReitzDistribution::new(0.0, 0.0);
            // let dielectric_bxdf = Box::new(DielectricBxdf::new(microfacet, 1.5));
            // let bsdf = Bsdf::new(dielectric_bxdf, Normal(hit_data.normal), Vec3::ZERO);

            if bsdf.flags().is_non_specular() {
                let ld = Self::sample_ld(
                    surface_interaction.clone(),
                    &bsdf,
                    wavelengths,
                    &mut sampler,
                    geometry_resources,
                );
                l += throughput * ld.0;
            }

            let wo = -Vec3::from(ray.D);
            let u = sampler.get_1d();
            if let Some(bsdf_sample) = bsdf.sample_f(
                wo,
                u,
                sampler.get_2d(),
                TransportMode::Radiance,
                BxdfReflTransFlags::all(),
            ) {
                throughput *= bsdf_sample.f.0
                    * bsdf_sample
                        .wi
                        .dot(surface_interaction.shading_normal.0)
                        .abs()
                    / bsdf_sample.pdf;

                p_b = if bsdf_sample.pdf_is_proportional {
                    bsdf.pdf(
                        wo,
                        bsdf_sample.wi,
                        TransportMode::Radiance,
                        BxdfReflTransFlags::all(),
                    )
                } else {
                    bsdf_sample.pdf
                };

                specular_bounce = bsdf_sample.flags.contains(BxdfFlags::SPECULAR);
                any_non_specular_bounces |= !bsdf_sample.flags.contains(BxdfFlags::SPECULAR);
                if bsdf_sample.flags.contains(BxdfFlags::TRANSMISSION) {
                    eta_scale *= sqr(bsdf_sample.eta);
                }
                prev_light_ctx = LightSourceSampleCtx::new_from_surface(surface_interaction);

                ray = Ray::new(interaction.point + bsdf_sample.wi * 0.0001, bsdf_sample.wi);
            } else {
                break;
            }

            let russian_roulette = throughput * eta_scale;
            if russian_roulette.max_element() < 1.0 && depth > 1 {
                let r = (1.0 - russian_roulette.max_element()).max(0.0);
                if sampler.get_1d() < r {
                    break;
                }
                throughput /= 1.0 - r;
            }
        }

        SampledSpectrum::new(l)
    }
}
