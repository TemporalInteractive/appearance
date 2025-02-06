use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use tinybvh::{vec_helpers::Vec3Helpers, Ray};

use crate::geometry_resources::GeometryResources;

pub struct CameraMatrices {
    pub inv_view: Mat4,
    pub inv_proj: Mat4,
}

pub fn render_pixel(
    uv: &Vec2,
    camera_matrices: &CameraMatrices,
    geometry_resources: &GeometryResources,
) -> Vec3 {
    let corrected_uv = Vec2::new(uv.x, -uv.y);
    let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
    let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

    let mut ray = Ray::new(origin.xyz(), direction.xyz());

    for _ in 0..1 {
        geometry_resources.tlas().intersect(&mut ray);
    }
    if ray.hit.t != 1e30 {
        let hit_data = geometry_resources.get_hit_data(&ray.hit);

        return hit_data.normal * 0.5 + 0.5;
    }

    let a = 0.5 * (ray.D.y() + 1.0);
    (1.0 - a) * Vec3::new(1.0, 1.0, 1.0) + a * Vec3::new(0.5, 0.7, 1.0)
}
