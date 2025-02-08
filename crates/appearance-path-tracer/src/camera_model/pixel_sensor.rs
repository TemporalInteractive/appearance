use crate::radiometry::{DenselySampledSpectrum, RgbColorSpace, Spectrum};

pub struct PixelSensor {
    r: DenselySampledSpectrum,
    g: DenselySampledSpectrum,
    b: DenselySampledSpectrum,
    imaging_ratio: f32,
}

impl PixelSensor {
    pub fn new(
        r: DenselySampledSpectrum,
        g: DenselySampledSpectrum,
        b: DenselySampledSpectrum,
        output_color_space: RgbColorSpace,
        sensor_illum: &dyn Spectrum,
        imaging_ratio: f32,
    ) -> Self {
        Self {
            r,
            g,
            b,
            imaging_ratio,
        }
    }
}
