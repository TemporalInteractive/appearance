use glam::{Mat4, UVec2, Vec2, Vec4, Vec4Swizzles};
use tinybvh::Ray;

use crate::{
    geometry_resources::GeometryResources,
    path_integrator::PathIntegrator,
    radiometry::{SampledSpectrum, SampledWavelengths},
    sampling::{zsobol_sampler::ZSobolSampler, Sampler},
};

/// All packets will be 16x16 because a packet of 256 rays can be traced with greater performance.
pub const PATH_TRACER_RAY_PACKET_SIZE: u32 = 16;
pub const RAYS_PER_PACKET: usize =
    (PATH_TRACER_RAY_PACKET_SIZE * PATH_TRACER_RAY_PACKET_SIZE) as usize;

pub struct CameraMatrices {
    pub inv_view: Mat4,
    pub inv_proj: Mat4,
}

#[derive(Default, Clone, Copy)]
pub struct SamplePixelResult {
    pub sampled_spectrum: SampledSpectrum,
    pub sampled_wavelengths: SampledWavelengths,
}

#[allow(clippy::too_many_arguments)]
pub fn render_pixels(
    uv: [Vec2; RAYS_PER_PACKET],
    seed: u64,
    camera_matrices: &CameraMatrices,
    geometry_resources: &GeometryResources,
    width: u32,
    height: u32,
    sample_idx: u32,
    sampels_per_pixel: u32,
) -> [SamplePixelResult; RAYS_PER_PACKET] {
    let path_integrator = PathIntegrator::new(4);

    let mut results = [SamplePixelResult::default(); RAYS_PER_PACKET];
    for i in 0..RAYS_PER_PACKET {
        let corrected_uv = Vec2::new(uv[i].x, -uv[i].y);
        let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
        let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

        let ray = Ray::new(origin.xyz(), direction.xyz());
        let mut sampler = Box::new(ZSobolSampler::new(
            sampels_per_pixel,
            UVec2::new(width, height),
            seed,
        ));

        let pixel = UVec2::new(
            ((corrected_uv.x * 0.5 + 0.5) * width as f32) as u32,
            ((corrected_uv.y * 0.5 + 0.5) * height as f32) as u32,
        );
        sampler.start_pixel_sample(pixel, sample_idx, 0);

        let wavelengths = SampledWavelengths::sample_visible(sampler.get_1d());

        results[i].sampled_spectrum =
            path_integrator.li(ray, &wavelengths, sampler, geometry_resources);
        results[i].sampled_wavelengths = wavelengths;
    }

    results
}
