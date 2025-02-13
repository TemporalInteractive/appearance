use glam::{Mat4, Vec2, Vec4, Vec4Swizzles};
use tinybvh::Ray;

use crate::{
    geometry_resources::GeometryResources,
    math::random::random_f32,
    path_integrator::PathIntegrator,
    radiometry::{SampledSpectrum, SampledWavelengths},
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

pub fn render_pixels(
    uv: [Vec2; RAYS_PER_PACKET],
    rng: [u32; RAYS_PER_PACKET],
    camera_matrices: &CameraMatrices,
    geometry_resources: &GeometryResources,
) -> [SamplePixelResult; RAYS_PER_PACKET] {
    let path_integrator = PathIntegrator::new(3);

    let mut results = [SamplePixelResult::default(); RAYS_PER_PACKET];
    for i in 0..RAYS_PER_PACKET {
        let corrected_uv = Vec2::new(uv[i].x, -uv[i].y);
        let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
        let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

        let ray = Ray::new(origin.xyz(), direction.xyz());
        let mut rng = rng[i];

        let wavelengths = SampledWavelengths::sample_visible(random_f32(&mut rng));

        results[i].sampled_spectrum =
            path_integrator.li(ray, &wavelengths, rng, geometry_resources);
        results[i].sampled_wavelengths = wavelengths;
    }

    results
}
