use core::convert::Into;
use std::rc::Rc;

use glam::{FloatExt, Vec3, Vec4};

use crate::math::Vec4Extensions;

use super::{
    black_body_emission, cie_x, cie_y, cie_z, Rgb, RgbColorSpace, RgbSigmoidPolynomial, Xyz,
    CIE_Y_INTEGRAL,
};

/// Minimum wavelength of visible light for humans.
pub const LAMBDA_MIN: f32 = 360.0;

/// Maximum wavelength of visible light for humans.
pub const LAMBDA_MAX: f32 = 830.0;

/// Represent values of the spectral distribution at discrete wavelengths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampledSpectrum(Vec4);

impl From<Vec4> for SampledSpectrum {
    fn from(v: Vec4) -> Self {
        SampledSpectrum(v)
    }
}

impl From<SampledSpectrum> for Vec4 {
    fn from(val: SampledSpectrum) -> Self {
        val.0
    }
}

impl SampledSpectrum {
    pub const N_SPECTRUM_SAMPLES: usize = 4;

    pub fn new(v: Vec4) -> Self {
        Self(v)
    }

    pub fn has_contribution(&self) -> bool {
        self.0.length_squared() > 0.0
    }

    pub fn to_xyz(&self, sampled_wavelengths: &SampledWavelengths) -> Xyz {
        let x = cie_x().sample(sampled_wavelengths);
        let y = cie_y().sample(sampled_wavelengths);
        let z = cie_z().sample(sampled_wavelengths);

        let xyz = Vec3::new(
            (x.0 * self.0).safe_div(sampled_wavelengths.pdf).avg(),
            (y.0 * self.0).safe_div(sampled_wavelengths.pdf).avg(),
            (z.0 * self.0).safe_div(sampled_wavelengths.pdf).avg(),
        ) / CIE_Y_INTEGRAL;

        Xyz::new(xyz)
    }

    pub fn to_rgb(
        &self,
        sampled_wavelengths: &SampledWavelengths,
        rgb_color_space: &RgbColorSpace,
    ) -> Rgb {
        let xyz = self.to_xyz(sampled_wavelengths);
        rgb_color_space.xyz_to_rgb(xyz)
    }
}

/// Stores the wavelengths for which a SampledSpectrum stores samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampledWavelengths {
    lambda: Vec4,
    pdf: Vec4,
}

impl SampledWavelengths {
    pub fn sample_uniform(u: f32) -> Self {
        let mut lambda = Vec4::ZERO;

        lambda[0] = LAMBDA_MIN.lerp(LAMBDA_MAX, u);

        let delta = (LAMBDA_MAX - LAMBDA_MIN) / 4.0;
        for i in 1..4 {
            lambda[i] = lambda[i - 1] + delta;
            if lambda[i] > LAMBDA_MAX {
                lambda[i] = LAMBDA_MIN + (lambda[i] - LAMBDA_MAX);
            }
        }

        let pdf = Vec4::splat(1.0 / (LAMBDA_MAX - LAMBDA_MIN));

        Self { lambda, pdf }
    }

    /// Terminates all but one of the wavelength.
    pub fn terminate_secondary(&mut self) {
        for i in 1..3 {
            self.pdf[i] = 0.0;
        }

        self.pdf[0] /= 4.0;
    }

    /// Are all but one wavelength already terminated?
    pub fn is_secondary_terminated(&self) -> bool {
        for i in 1..3 {
            if self.pdf[i] != 0.0 {
                return false;
            }
        }
        true
    }
}

/// Represents a range of spectral sample values.
pub trait Spectrum: std::fmt::Debug {
    fn spectral_distribution(&self, lambda: f32) -> f32;
    fn max_spectral_distribution(&self) -> f32;

    fn sample(&self, sampled_wavelengths: &SampledWavelengths) -> SampledSpectrum {
        SampledSpectrum::new(Vec4::new(
            self.spectral_distribution(sampled_wavelengths.lambda[0]),
            self.spectral_distribution(sampled_wavelengths.lambda[1]),
            self.spectral_distribution(sampled_wavelengths.lambda[2]),
            self.spectral_distribution(sampled_wavelengths.lambda[3]),
        ))
    }
}

pub fn spectrum_inner_product(a: &dyn Spectrum, b: &dyn Spectrum) -> f32 {
    let mut integral = 0.0;

    for lambda in LAMBDA_MIN as u32..LAMBDA_MAX as u32 {
        integral += a.spectral_distribution(lambda as f32) * b.spectral_distribution(lambda as f32);
    }

    integral
}

/// Represents a constant spectral distribution over all wavelengths.
#[derive(Debug, Clone)]
pub struct ConstantSpectrum {
    spectral_distribution: f32,
}

impl ConstantSpectrum {
    pub fn new(spectral_distribution: f32) -> Self {
        Self {
            spectral_distribution,
        }
    }
}

impl Spectrum for ConstantSpectrum {
    fn spectral_distribution(&self, _lambda: f32) -> f32 {
        self.spectral_distribution
    }

    fn max_spectral_distribution(&self) -> f32 {
        self.spectral_distribution
    }
}

/// Stores a spectral distribution sampled at 1 nm intervals over a given range of integer wavelengths.
#[derive(Debug, Clone)]
pub struct DenselySampledSpectrum {
    lambda_min: u32,
    lambda_max: u32,
    spectral_distribution: Vec<f32>,
}

impl DenselySampledSpectrum {
    pub fn new(lambda_min: u32, lambda_max: u32) -> Self {
        Self {
            lambda_min,
            lambda_max,
            spectral_distribution: vec![0.0; (lambda_max - lambda_min) as usize],
        }
    }

    pub fn new_from_spectrum(spectrum: &dyn Spectrum, lambda_min: u32, lambda_max: u32) -> Self {
        let mut spectral_distribution = Vec::with_capacity((lambda_max - lambda_min) as usize);

        for lambda in lambda_min..lambda_max {
            spectral_distribution.push(spectrum.spectral_distribution(lambda as f32));
        }

        Self {
            lambda_min,
            lambda_max,
            spectral_distribution,
        }
    }

    pub fn new_from_spectral_distribution(
        spectral_distribution: Vec<f32>,
        lambda_min: u32,
        lambda_max: u32,
    ) -> Self {
        Self {
            lambda_min,
            lambda_max,
            spectral_distribution,
        }
    }
}

impl Spectrum for DenselySampledSpectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32 {
        let i = lambda.round() as i32 - self.lambda_min as i32;
        if i < 0 || i >= self.spectral_distribution.len() as i32 {
            0.0
        } else {
            self.spectral_distribution[i as usize]
        }
    }

    fn max_spectral_distribution(&self) -> f32 {
        *self.spectral_distribution.last().unwrap()
    }
}

/// Gives the spectral distribution of a blackbody emitter at a specified temperature.
#[derive(Debug, Clone)]
pub struct BlackBodySpectrum {
    t: f32,
    normalization_factor: f32,
}

impl BlackBodySpectrum {
    pub fn new(t: f32) -> Self {
        let lambda_max = 2.897_772e-3 / t;
        let normalization_factor = 1.0 / black_body_emission(lambda_max * 1e9, t);

        Self {
            t,
            normalization_factor,
        }
    }
}

impl Spectrum for BlackBodySpectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32 {
        black_body_emission(lambda, self.t) * self.normalization_factor
    }

    fn max_spectral_distribution(&self) -> f32 {
        1.0
    }
}

/// Handles rgb values in the range of [0, 1] in a given rgb color space
#[derive(Debug, Clone)]
pub struct RgbAlbedoSpectrum {
    polynomial: RgbSigmoidPolynomial,
}

impl RgbAlbedoSpectrum {
    pub fn new(rgb: Rgb, color_space: &RgbColorSpace) -> Self {
        let polynomial = color_space.rgb_to_polynomial(rgb);

        Self { polynomial }
    }
}

impl Spectrum for RgbAlbedoSpectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32 {
        self.polynomial.evaluate(lambda)
    }

    fn max_spectral_distribution(&self) -> f32 {
        self.polynomial.max_value()
    }
}

/// Handles rgb values in the range of [0, INF] in a given rgb color space
#[derive(Debug, Clone)]
pub struct RgbUnboundedSpectrum {
    polynomial: RgbSigmoidPolynomial,
    scale: f32,
}

impl RgbUnboundedSpectrum {
    pub fn new(rgb: Rgb, color_space: &RgbColorSpace) -> Self {
        let m = Vec3::from(rgb).max_element();
        let scale = 2.0 * m;
        let polynomial = if scale != 0.0 {
            color_space.rgb_to_polynomial(Rgb::new(Vec3::from(rgb) / scale))
        } else {
            color_space.rgb_to_polynomial(Rgb::new(Vec3::ZERO))
        };

        Self { scale, polynomial }
    }
}

impl Spectrum for RgbUnboundedSpectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32 {
        self.scale * self.polynomial.evaluate(lambda)
    }

    fn max_spectral_distribution(&self) -> f32 {
        self.scale * self.polynomial.max_value()
    }
}

#[derive(Debug, Clone)]
pub struct RgbIlluminantSpectrum {
    polynomial: RgbSigmoidPolynomial,
    scale: f32,
    illuminant: Rc<dyn Spectrum>,
}

impl RgbIlluminantSpectrum {
    pub fn new(rgb: Rgb, color_space: &RgbColorSpace, illuminant: Rc<dyn Spectrum>) -> Self {
        let m = Vec3::from(rgb).max_element();
        let scale = 2.0 * m;
        let polynomial = if scale != 0.0 {
            color_space.rgb_to_polynomial(Rgb::new(Vec3::from(rgb) / scale))
        } else {
            color_space.rgb_to_polynomial(Rgb::new(Vec3::ZERO))
        };

        Self {
            scale,
            polynomial,
            illuminant,
        }
    }
}

impl Spectrum for RgbIlluminantSpectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32 {
        self.scale
            * self.polynomial.evaluate(lambda)
            * self.illuminant.spectral_distribution(lambda)
    }

    fn max_spectral_distribution(&self) -> f32 {
        self.scale * self.polynomial.max_value() * self.illuminant.max_spectral_distribution()
    }

    fn sample(&self, sampled_wavelengths: &SampledWavelengths) -> SampledSpectrum {
        SampledSpectrum::new(
            Vec4::new(
                self.spectral_distribution(sampled_wavelengths.lambda[0]),
                self.spectral_distribution(sampled_wavelengths.lambda[1]),
                self.spectral_distribution(sampled_wavelengths.lambda[2]),
                self.spectral_distribution(sampled_wavelengths.lambda[3]),
            ) * Vec4::from(self.illuminant.sample(sampled_wavelengths)),
        )
    }
}
