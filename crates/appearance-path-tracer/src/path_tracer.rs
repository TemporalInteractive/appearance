use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use tinybvh::{vec_helpers::Vec3Helpers, Ray};

use crate::{
    geometry_resources::GeometryResources,
    math::random::random_f32,
    radiometry::{RgbColorSpace, SampledSpectrum, SampledWavelengths},
};

/// All packets will be 16x16 because a packet of 256 rays can be traced with greater performance.
pub const PATH_TRACER_RAY_PACKET_SIZE: u32 = 16;
pub const RAYS_PER_PACKET: usize =
    (PATH_TRACER_RAY_PACKET_SIZE * PATH_TRACER_RAY_PACKET_SIZE) as usize;

pub struct CameraMatrices {
    pub inv_view: Mat4,
    pub inv_proj: Mat4,
}

pub fn render_pixels(
    uv: [Vec2; RAYS_PER_PACKET],
    _rng: [u32; RAYS_PER_PACKET],
    camera_matrices: &CameraMatrices,
    geometry_resources: &GeometryResources,
) -> [Vec3; RAYS_PER_PACKET] {
    let mut rays = vec![];

    for y in 0..PATH_TRACER_RAY_PACKET_SIZE {
        for x in 0..PATH_TRACER_RAY_PACKET_SIZE {
            let i = (y * PATH_TRACER_RAY_PACKET_SIZE + x) as usize;
            let uv = uv[i];

            let corrected_uv = Vec2::new(uv.x, -uv.y);
            let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
            let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
            let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

            rays.push(Ray::new(origin.xyz(), direction.xyz()));
        }
    }

    let mut rays: [Ray; RAYS_PER_PACKET] = rays.try_into().unwrap_or_else(|v: Vec<Ray>| {
        panic!(
            "Expected a Vec of length {} but it was {}",
            RAYS_PER_PACKET,
            v.len()
        )
    });

    //let _color_space = RgbColorSpace::srgb();

    // TODO: waiting for tinybvh patch for using ray packets with TLASES
    // geometry_resources.tlas().intersect_256(&mut rays);
    for ray in &mut rays {
        geometry_resources.tlas().intersect(ray);
    }

    let mut colors = [Vec3::ZERO; RAYS_PER_PACKET];
    for i in 0..RAYS_PER_PACKET {
        let ray = &rays[i];

        if ray.hit.t != 1e30 {
            let hit_data = geometry_resources.get_hit_data(&ray.hit);

            colors[i] = hit_data.normal * 0.5 + 0.5;
        } else {
            let a = 0.5 * (ray.D.y() + 1.0);
            colors[i] = (1.0 - a) * Vec3::new(1.0, 1.0, 1.0) + a * Vec3::new(0.5, 0.7, 1.0);
        }
    }

    colors
}
