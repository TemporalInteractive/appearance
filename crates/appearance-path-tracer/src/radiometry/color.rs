use glam::Vec3;

use super::{
    spectrum_inner_product, DenselySampledSpectrum, SampledSpectrum, Spectrum, LAMBDA_MAX,
    LAMBDA_MIN,
};

pub const CIE_Y_INTEGRAL: f32 = 106.856895;

pub fn cie_x() -> DenselySampledSpectrum {
    DenselySampledSpectrum::new(LAMBDA_MIN as u32, LAMBDA_MAX as u32)
}

pub fn cie_y() -> DenselySampledSpectrum {
    DenselySampledSpectrum::new(LAMBDA_MIN as u32, LAMBDA_MAX as u32)
}

pub fn cie_z() -> DenselySampledSpectrum {
    DenselySampledSpectrum::new(LAMBDA_MIN as u32, LAMBDA_MAX as u32)
}

pub struct Xyz(Vec3);

impl From<Vec3> for Xyz {
    fn from(v: Vec3) -> Self {
        Xyz(v)
    }
}

impl From<Xyz> for Vec3 {
    fn from(val: Xyz) -> Self {
        val.0
    }
}

impl Xyz {
    pub fn new(xyz: Vec3) -> Self {
        Self(xyz)
    }

    pub fn from_spectrum(spectrum: &dyn Spectrum) -> Self {
        let xyz = Vec3::new(
            spectrum_inner_product(&cie_x(), spectrum),
            spectrum_inner_product(&cie_y(), spectrum),
            spectrum_inner_product(&cie_z(), spectrum),
        ) / CIE_Y_INTEGRAL;

        Self::new(xyz)
    }
}
