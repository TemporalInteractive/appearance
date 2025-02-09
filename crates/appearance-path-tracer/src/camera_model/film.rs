use std::sync::Arc;

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

unsafe impl Send for PixelDataPtr {}
unsafe impl Sync for PixelDataPtr {}

pub struct Film {
    resolution: UVec2,
    sensor: PixelSensor,
    rgb_color_space: Arc<RgbColorSpace>,
    output_rgb_from_sensor_rgb: Mat3,

    pixels: Vec<u8>,
    pixel_ptr: PixelDataPtr,
}

impl Film {
    pub fn new(
        resolution: UVec2,
        sensor: PixelSensor,
        rgb_color_space: Arc<RgbColorSpace>,
    ) -> Self {
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
        pixel_idx: usize,
        //uv_film: Vec2,
        sampled_spectrum: &SampledSpectrum,
        wavelengths: &SampledWavelengths,
        //weight: f32,
    ) {
        // TODO: ok so the pdf is low? resulting in exploded rgbs, I want to visualize the image after a few samples not after (LAMBDA_MAX - LAMBDA_MIN) samples
        let rgb = self.sensor.to_sensor_rgb(sampled_spectrum, wavelengths);

        // TODO: should be applied after adding all samples together
        let rgb = Rgb::new(self.output_rgb_from_sensor_rgb * rgb.0);

        // TODO: optionally clamp rgb

        // TODO: accumulate samples instead of overwriting!
        self.pixel_ptr.write_pixel(pixel_idx, rgb);
    }

    pub fn sample_wavelengths(&self, u: f32) -> SampledWavelengths {
        SampledWavelengths::sample_visible(u)
    }

    pub fn resize(&mut self, resolution: UVec2) {
        self.resolution = resolution;
        self.pixels = vec![0u8; (resolution.x * resolution.y * 3) as usize];
        self.pixel_ptr = PixelDataPtr::new(&mut self.pixels);
    }

    pub fn resolution(&self) -> UVec2 {
        self.resolution
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}
