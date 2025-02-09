use glam::Vec2;
use tinybvh::Ray;

pub mod film;
pub mod perspective;
pub mod pixel_sensor;

use crate::radiometry::{SampledSpectrum, SampledWavelengths};

pub struct CameraSample {
    /// Point on the film to sample
    pub film_uv: Vec2,
    /// Point on the lens to sample
    pub lens_uv: Vec2,
}

pub struct CameraRay {
    pub ray: Ray,
    pub weight: SampledSpectrum,
}

pub trait CameraModel {
    fn generate_ray(
        &self,
        sample: &CameraSample,
        wavelengths: &mut SampledWavelengths,
    ) -> Option<CameraRay>;
}
