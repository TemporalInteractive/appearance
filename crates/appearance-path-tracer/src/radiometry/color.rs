use std::sync::OnceLock;

use glam::{Mat3, Vec2, Vec3};

use crate::math::sqr;

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

/// XYZ color space, a device-independent color space, which means that it does not describe the characteristics of a particular display or color measurement device.
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

    pub fn from_xy(xy: Vec2) -> Self {
        if xy.y == 0.0 {
            Self(Vec3::ZERO)
        } else {
            let y_lambda = 1.0;
            Self(Vec3::new(
                xy.x * y_lambda / xy.y,
                y_lambda,
                (1.0 - xy.x - xy.y) * y_lambda / xy.y,
            ))
        }
    }

    pub fn to_xy(&self) -> Vec2 {
        let sum = self.0.element_sum();
        Vec2::new(self.0.x / sum, self.0.y / sum)
    }
}

/// RGB color space defined by using the chromaticities of red, green, and blue color primaries.
pub struct Rgb(Vec3);

impl From<Vec3> for Rgb {
    fn from(v: Vec3) -> Self {
        Rgb(v)
    }
}

impl From<Rgb> for Vec3 {
    fn from(val: Rgb) -> Self {
        val.0
    }
}

impl Rgb {
    pub fn new(rgb: Vec3) -> Self {
        Self(rgb)
    }
}

pub struct RgbColorSpace {
    xyz_from_rgb: Mat3,
    rgb_from_xyz: Mat3,

    r: Vec2,
    g: Vec2,
    b: Vec2,
    w: Vec2,
}

impl RgbColorSpace {
    pub fn new(r_xy: Vec2, g_xy: Vec2, b_xy: Vec2, illuminant: &dyn Spectrum) -> Self {
        let w = Xyz::from_spectrum(illuminant);
        let w_xy = w.to_xy();

        let r = Xyz::from_xy(r_xy);
        let g = Xyz::from_xy(g_xy);
        let b = Xyz::from_xy(b_xy);
        let rgb = Mat3::from_cols_array(&[
            r.0.x, g.0.x, b.0.x, r.0.y, g.0.y, b.0.y, r.0.z, g.0.z, b.0.z,
        ]);

        let c = Xyz::new(rgb.inverse() * w.0);
        let xyz_from_rgb = rgb * Mat3::from_diagonal(c.0);
        let rgb_from_xyz = xyz_from_rgb.inverse();

        Self {
            xyz_from_rgb,
            rgb_from_xyz,
            r: r_xy,
            g: g_xy,
            b: b_xy,
            w: w_xy,
        }
    }

    pub fn xyz_to_rgb(&self, xyz: Xyz) -> Rgb {
        Rgb::new(self.rgb_from_xyz * xyz.0)
    }

    pub fn rgb_to_xyz(&self, rgb: Rgb) -> Xyz {
        Xyz::new(self.xyz_from_rgb * rgb.0)
    }
}

pub struct RgbSigmoidPolynomial {
    c0: f32,
    c1: f32,
    c2: f32,
}

impl RgbSigmoidPolynomial {
    pub fn new(c0: f32, c1: f32, c2: f32) -> Self {
        Self { c0, c1, c2 }
    }

    pub fn evaluate(&self, lambda: f32) -> f32 {
        let polynomial = lambda * (lambda * self.c0 + self.c1) + self.c2;

        if polynomial.is_infinite() {
            if polynomial > 0.0 {
                1.0
            } else {
                0.0
            }
        } else {
            0.5 + polynomial / (2.0 * (1.0 + sqr(polynomial)).sqrt())
        }
    }

    pub fn max_value(&self) -> f32 {
        let result = self.evaluate(360.0).max(self.evaluate(830.0));
        let lambda = -self.c1 / (2.0 * self.c0);

        if (360.0..=830.0).contains(&lambda) {
            result.max(self.evaluate(lambda))
        } else {
            result
        }
    }
}
