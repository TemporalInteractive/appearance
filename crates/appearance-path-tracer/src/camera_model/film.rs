use glam::{Mat3, UVec2, Vec2, Vec3};

use crate::radiometry::{Rgb, RgbColorSpace, SampledSpectrum, SampledWavelengths};

use super::pixel_sensor::PixelSensor;

/// Enables access to pixel data from multiple threads without any safety checks.
struct PixelDataPtr(*mut u8);

impl PixelDataPtr {
    fn new(pixels: &mut Vec<u8>) -> Self {
        Self(pixels.as_mut_ptr())
    }

    unsafe fn write_pixel(&self, i: usize, rgb: Rgb) {
        *self.0.add(i * 3) = (rgb.0.x * 255.0) as u8;
        *self.0.add(i * 3 + 1) = (rgb.0.y * 255.0) as u8;
        *self.0.add(i * 3 + 2) = (rgb.0.z * 255.0) as u8;
    }
}

pub struct Film {
    resolution: UVec2,
    sensor: PixelSensor,
    rgb_color_space: RgbColorSpace,
    output_rgb_from_sensor_rgb: Mat3,

    pixels: Vec<u8>,
    pixel_ptr: PixelDataPtr,
}

impl Film {
    pub fn new(resolution: UVec2, sensor: PixelSensor, rgb_color_space: RgbColorSpace) -> Self {
        let output_rgb_from_sensor_rgb =
            *rgb_color_space.rgb_from_xyz_mat3() * *sensor.xyz_from_sensor_rgb_mat3();

        let mut pixels = vec![0u8; (resolution.x * resolution.y * 3) as usize];
        let pixel_ptr = PixelDataPtr::new(&mut pixels);

        Self {
            resolution,
            sensor,
            rgb_color_space,
            output_rgb_from_sensor_rgb,
            pixels,
            pixel_ptr,
        }
    }

    pub unsafe fn add_sample(
        &self,
        uv_film: Vec2,
        sampled_spectrum: &SampledSpectrum,
        wavelengths: &SampledWavelengths,
        weight: f32,
    ) {
        let rgb = self.sensor.to_sensor_rgb(sampled_spectrum, wavelengths);

        let x = (uv_film.x * self.resolution.x as f32) as usize;
        let y = (uv_film.y * self.resolution.y as f32) as usize;
        let i = y * self.resolution.x as usize + x;

        // TODO: optionally clamp rgb

        // TODO: accumulate samples instead of overwriting!
        self.pixel_ptr.write_pixel(i, rgb);
    }

    pub fn sample_wavelengths(&self, u: f32) -> SampledWavelengths {
        SampledWavelengths::sample_visible(u)
    }

    pub fn resolution(&self) -> UVec2 {
        self.resolution
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}
