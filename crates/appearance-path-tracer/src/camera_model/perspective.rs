use glam::{Mat4, Vec2, Vec4, Vec4Swizzles};
use tinybvh::Ray;

use crate::radiometry::{SampledSpectrum, SampledWavelengths};

use super::{CameraModel, CameraRay, CameraSample};

pub struct PerspectiveCamera {
    inv_view: Mat4,
    inv_proj: Mat4,
}

impl PerspectiveCamera {
    pub fn new(inv_view: Mat4, inv_proj: Mat4) -> Self {
        Self { inv_view, inv_proj }
    }
}

impl CameraModel for PerspectiveCamera {
    fn generate_ray(
        &self,
        sample: &CameraSample,
        _wavelengths: &mut SampledWavelengths,
    ) -> Option<CameraRay> {
        let corrected_uv = Vec2::new(sample.film_uv.x, -sample.film_uv.y);
        let origin = self.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let target = self.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
        let direction = self.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

        Some(CameraRay {
            ray: Ray::new(origin.xyz(), direction.xyz()),
            weight: SampledSpectrum::new(Vec4::ZERO),
        })
    }
}
