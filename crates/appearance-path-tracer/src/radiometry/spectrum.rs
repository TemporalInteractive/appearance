use glam::{FloatExt, Vec4};

use super::black_body_emission;

/// Minimum wavelength of visible light for humans.
pub const LAMBDA_MIN: f32 = 360.0;

/// Maximum wavelength of visible light for humans.
pub const LAMBDA_MAX: f32 = 830.0;

/// Represent values of the spectral distribution at discrete wavelengths.
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
}

/// Stores the wavelengths for which a SampledSpectrum stores samples.
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
pub trait Spectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32;
    fn max_spectral_distribution(&self) -> f32;
}

/// Represents a constant spectral distribution over all wavelengths.
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
pub struct DenselySampledSpectrum {
    lambda_min: u32,
    lambda_max: u32,
    spectral_distribution: Vec<f32>,
}

impl DenselySampledSpectrum {
    pub fn empty(lambda_min: u32, lambda_max: u32) -> Self {
        Self {
            lambda_min,
            lambda_max,
            spectral_distribution: vec![0.0; (lambda_max - lambda_min) as usize],
        }
    }

    pub fn new(spectrum: &dyn Spectrum, lambda_min: u32, lambda_max: u32) -> Self {
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
