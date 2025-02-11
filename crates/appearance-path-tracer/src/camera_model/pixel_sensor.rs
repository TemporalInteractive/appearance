use glam::{Mat3, Vec3, Vec4};

use crate::{
    math::{safe_div, Mat3Extensions, Vec4Extensions},
    radiometry::{
        data_tables::swatch_reflectances::N_SWATCH_REFLECTANCES, project_reflectance,
        DenselySampledSpectrum, PiecewiseLinearSpectrum, Rgb, RgbColorSpace, SampledSpectrum,
        SampledWavelengths, Spectrum,
    },
};

pub struct PixelSensor {
    xyz_from_sensor_rgb: Mat3,
    r: PiecewiseLinearSpectrum,
    g: PiecewiseLinearSpectrum,
    b: PiecewiseLinearSpectrum,
    imaging_ratio: f32,
}

impl PixelSensor {
    pub fn new(
        r: PiecewiseLinearSpectrum,
        g: PiecewiseLinearSpectrum,
        b: PiecewiseLinearSpectrum,
        output_color_space: &RgbColorSpace,
        sensor_illum: &dyn Spectrum,
        imaging_ratio: f32,
    ) -> Self {
        let mut rgb_camera = [Vec3::ZERO; N_SWATCH_REFLECTANCES];
        for (i, rbg_camera) in rgb_camera.iter_mut().enumerate() {
            *rbg_camera = project_reflectance(
                &PiecewiseLinearSpectrum::swatch_reflectances()[i],
                sensor_illum,
                &r,
                &g,
                &b,
            );
        }

        let mut xyz_output = [Vec3::ZERO; N_SWATCH_REFLECTANCES];
        let sensor_white_g = sensor_illum.inner_product(&g);
        let sensor_white_y = sensor_illum.inner_product(DenselySampledSpectrum::cie_y());
        for (i, xyz_output) in xyz_output.iter_mut().enumerate() {
            *xyz_output = project_reflectance(
                &PiecewiseLinearSpectrum::swatch_reflectances()[i],
                output_color_space.illuminant().as_ref(),
                DenselySampledSpectrum::cie_x(),
                DenselySampledSpectrum::cie_y(),
                DenselySampledSpectrum::cie_z(),
            ) * (sensor_white_y / sensor_white_g);
        }

        let xyz_from_sensor_rgb = Mat3::linear_least_squares(&rgb_camera, &xyz_output);

        Self {
            xyz_from_sensor_rgb,
            r,
            g,
            b,
            imaging_ratio,
        }
    }

    pub fn xyz_from_sensor_rgb_mat3(&self) -> &Mat3 {
        &self.xyz_from_sensor_rgb
    }

    pub fn to_sensor_rgb(
        &self,
        sampled_spectrum: &SampledSpectrum,
        wavelengths: &SampledWavelengths,
    ) -> Rgb {
        let sampled_spectrum = Vec4::new(
            safe_div(sampled_spectrum.0.x, wavelengths.pdf().x),
            safe_div(sampled_spectrum.0.y, wavelengths.pdf().y),
            safe_div(sampled_spectrum.0.z, wavelengths.pdf().z),
            safe_div(sampled_spectrum.0.w, wavelengths.pdf().w),
        );

        Rgb::new(
            Vec3::new(
                (self.r.sample(wavelengths).0 * sampled_spectrum).avg(),
                (self.g.sample(wavelengths).0 * sampled_spectrum).avg(),
                (self.b.sample(wavelengths).0 * sampled_spectrum).avg(),
            ) * self.imaging_ratio,
        )
    }
}
