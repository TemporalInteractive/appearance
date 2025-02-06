use std::sync::OnceLock;

use glam::Vec3;

use super::{
    data_tables::{CIE_X, CIE_Y, CIE_Z},
    spectrum_inner_product, DenselySampledSpectrum, SampledSpectrum, Spectrum, LAMBDA_MAX,
    LAMBDA_MIN,
};

pub const CIE_Y_INTEGRAL: f32 = 106.856895;

static CIE_X_SPECTRUM: OnceLock<DenselySampledSpectrum> = OnceLock::new();
static CIE_Y_SPECTRUM: OnceLock<DenselySampledSpectrum> = OnceLock::new();
static CIE_Z_SPECTRUM: OnceLock<DenselySampledSpectrum> = OnceLock::new();

pub fn cie_x() -> &'static DenselySampledSpectrum {
    CIE_X_SPECTRUM.get_or_init(|| {
        DenselySampledSpectrum::new_from_spectral_distribution(
            CIE_X.to_vec(),
            LAMBDA_MIN as u32,
            LAMBDA_MAX as u32,
        )
    })
}

pub fn cie_y() -> &'static DenselySampledSpectrum {
    CIE_Y_SPECTRUM.get_or_init(|| {
        DenselySampledSpectrum::new_from_spectral_distribution(
            CIE_Y.to_vec(),
            LAMBDA_MIN as u32,
            LAMBDA_MAX as u32,
        )
    })
}

pub fn cie_z() -> &'static DenselySampledSpectrum {
    CIE_Z_SPECTRUM.get_or_init(|| {
        DenselySampledSpectrum::new_from_spectral_distribution(
            CIE_Z.to_vec(),
            LAMBDA_MIN as u32,
            LAMBDA_MAX as u32,
        )
    })
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
            spectrum_inner_product(cie_x(), spectrum),
            spectrum_inner_product(cie_y(), spectrum),
            spectrum_inner_product(cie_z(), spectrum),
        ) / CIE_Y_INTEGRAL;

        Self::new(xyz)
    }
}
