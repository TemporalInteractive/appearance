use std::sync::{Arc, OnceLock};

use glam::{FloatExt, Vec3, Vec4};

use crate::{
    math::{find_interval_fast, lookup_table::LookupTable, sqr, Vec4Extensions},
    radiometry::data_tables::cie::{CIE_S0, CIE_S1, CIE_S2, CIE_SAMPLES, CIE_S_LAMBDA},
};

use super::{
    black_body_emission,
    data_tables::{
        camera::{CANON_EOS_100D_B, CANON_EOS_100D_G, CANON_EOS_100D_R},
        cie::{
            ACES_ILLUM_D60, CIE_ILLUM_D5000, CIE_ILLUM_D6500, CIE_ILLUM_F1, CIE_ILLUM_F10,
            CIE_ILLUM_F11, CIE_ILLUM_F12, CIE_ILLUM_F2, CIE_ILLUM_F3, CIE_ILLUM_F4, CIE_ILLUM_F5,
            CIE_ILLUM_F6, CIE_ILLUM_F7, CIE_ILLUM_F8, CIE_ILLUM_F9, CIE_LAMBDA, CIE_X, CIE_Y,
            CIE_Z,
        },
        swatch_reflectances::SWATCH_REFLECTANCES,
    },
    Rgb, RgbColorSpace, RgbSigmoidPolynomial, Xyz, CIE_Y_INTEGRAL,
};

/// Minimum wavelength of visible light for humans.
pub const LAMBDA_MIN: f32 = 360.0;

/// Maximum wavelength of visible light for humans.
pub const LAMBDA_MAX: f32 = 830.0;

/// Represent values of the spectral distribution at discrete wavelengths.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SampledSpectrum(pub Vec4);

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

    pub fn to_xyz(self, sampled_wavelengths: &SampledWavelengths) -> Xyz {
        let x = DenselySampledSpectrum::cie_x().sample(sampled_wavelengths);
        let y = DenselySampledSpectrum::cie_y().sample(sampled_wavelengths);
        let z = DenselySampledSpectrum::cie_z().sample(sampled_wavelengths);

        let xyz = Vec3::new(
            (x.0 * self.0).safe_div(sampled_wavelengths.pdf).avg(),
            (y.0 * self.0).safe_div(sampled_wavelengths.pdf).avg(),
            (z.0 * self.0).safe_div(sampled_wavelengths.pdf).avg(),
        ) / CIE_Y_INTEGRAL;

        Xyz::new(xyz)
    }

    pub fn to_rgb(
        self,
        sampled_wavelengths: &SampledWavelengths,
        rgb_color_space: &RgbColorSpace,
    ) -> Rgb {
        let xyz = self.to_xyz(sampled_wavelengths);
        rgb_color_space.xyz_to_rgb(xyz)
    }
}

/// Stores the wavelengths for which a SampledSpectrum stores samples.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SampledWavelengths {
    lambda: Vec4,
    pdf: Vec4,
}

static VISIBLE_WAVELENGTHS_PDF_LOOKUP: OnceLock<LookupTable> = OnceLock::new();
static SAMPLE_VISIBLE_WAVELENGTHS_LOOKUP: OnceLock<LookupTable> = OnceLock::new();

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

        // TODO: why can't I wrap my head around this pdf?
        //let pdf = Vec4::splat(1.0 / (LAMBDA_MAX - LAMBDA_MIN));
        let pdf = Vec4::ONE;

        Self { lambda, pdf }
    }

    pub fn sample_visible(u: f32) -> Self {
        let mut lambda = Vec4::ZERO;
        let mut pdf = Vec4::ZERO;

        for i in 0..4 {
            let mut up = u + (i as f32 / 4.0);
            if up > 1.0 {
                up -= 1.0;
            }

            lambda[i] = Self::sample_visible_wavelengths(up);
            pdf[i] = Self::visible_wavelengths_pdf(lambda[i]);
        }

        Self { lambda, pdf }
    }

    fn visible_wavelengths_pdf(wavelength: f32) -> f32 {
        if !(360.0..=830.0).contains(&wavelength) {
            0.0
        } else {
            let lookup = VISIBLE_WAVELENGTHS_PDF_LOOKUP.get_or_init(|| {
                LookupTable::new(
                    |x| {
                        // TODO: Again, why can't I wrap my head around this pdf?
                        (0.003_939_804 / sqr((0.0072 * (x - 538.0)).cosh()))
                            * (LAMBDA_MAX - LAMBDA_MIN)
                    },
                    360.0,
                    830.0,
                    10.0,
                )
            });

            lookup.evaluate(wavelength)
        }
    }

    fn sample_visible_wavelengths(u: f32) -> f32 {
        let lookup = SAMPLE_VISIBLE_WAVELENGTHS_LOOKUP.get_or_init(|| {
            LookupTable::new(
                |x| 538.0 - 138.888_89 * (0.85691062 - 1.827_502 * x).atanh(),
                0.0,
                1.0,
                1000.0,
            )
        });

        lookup.evaluate(u)
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

    pub fn wavelengths(&self) -> &Vec4 {
        &self.lambda
    }

    pub fn pdf(&self) -> &Vec4 {
        &self.pdf
    }
}

/// Represents a range of spectral sample values.
pub trait Spectrum: std::fmt::Debug + Sync + Send {
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

    fn inner_product(&self, other: &dyn Spectrum) -> f32 {
        let mut integral = 0.0;

        for lambda in LAMBDA_MIN as u32..=LAMBDA_MAX as u32 {
            integral += self.spectral_distribution(lambda as f32)
                * other.spectral_distribution(lambda as f32);
        }

        integral
    }
}

pub fn project_reflectance(
    refl: &dyn Spectrum,
    illum: &dyn Spectrum,
    b1: &dyn Spectrum,
    b2: &dyn Spectrum,
    b3: &dyn Spectrum,
) -> Vec3 {
    let mut integral = 0.0;
    let mut result = Vec3::ZERO;

    for lambda in LAMBDA_MIN as u32..=LAMBDA_MAX as u32 {
        let lambda = lambda as f32;

        let illum_spectral_distribution = illum.spectral_distribution(lambda);
        let refl_illum_spectral_distribution =
            refl.spectral_distribution(lambda) * illum_spectral_distribution;

        integral += b2.spectral_distribution(lambda) * illum_spectral_distribution;
        result.x += b1.spectral_distribution(lambda) * refl_illum_spectral_distribution;
        result.y += b2.spectral_distribution(lambda) * refl_illum_spectral_distribution;
        result.z += b3.spectral_distribution(lambda) * refl_illum_spectral_distribution;
    }

    result / integral
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

static SWATCH_REFLECTANCES_SPECTRUM: OnceLock<[PiecewiseLinearSpectrum; 24]> = OnceLock::new();

static CIE_X_SPECTRUM_LPW: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_Y_SPECTRUM_LPW: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_Z_SPECTRUM_LPW: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();

static CIE_ILLUM_D5000_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static ACES_ILLUM_D60_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_D6500_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F1_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F2_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F3_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F4_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F5_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F6_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F7_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F8_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F9_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F10_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F11_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CIE_ILLUM_F12_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();

static CANON_EOS_100D_R_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CANON_EOS_100D_G_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();
static CANON_EOS_100D_B_SPECTRUM: OnceLock<PiecewiseLinearSpectrum> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct PiecewiseLinearSpectrum {
    reflectance: Vec<f32>,
    wavelengths: Vec<f32>,
    max_reflectance: f32,
}

impl PiecewiseLinearSpectrum {
    pub fn new(reflectance: Vec<f32>, wavelengths: Vec<f32>) -> Self {
        assert!(reflectance.len() == wavelengths.len());
        let max_reflectance = reflectance.iter().cloned().fold(0.0, f32::max);

        Self {
            reflectance,
            wavelengths,
            max_reflectance,
        }
    }

    pub fn from_interleaved(reflectance_and_wavelengths: &[f32], normalize: bool) -> Self {
        assert!(reflectance_and_wavelengths.len() % 2 == 0);
        let num_wavelengths = reflectance_and_wavelengths.len() / 2;

        let mut reflectance = Vec::with_capacity(num_wavelengths);
        let mut wavelengths = Vec::with_capacity(num_wavelengths);

        if reflectance_and_wavelengths[0] > LAMBDA_MIN {
            wavelengths.push(LAMBDA_MIN - 1.0);
            reflectance.push(reflectance_and_wavelengths[1]);
        }
        for i in 0..num_wavelengths {
            wavelengths.push(reflectance_and_wavelengths[i * 2]);
            reflectance.push(reflectance_and_wavelengths[i * 2 + 1]);
        }
        if *wavelengths.last().unwrap() < LAMBDA_MAX {
            wavelengths.push(LAMBDA_MAX + 1.0);
            reflectance.push(*reflectance.last().unwrap());
        }

        let max_reflectance = reflectance.iter().cloned().fold(0.0, f32::max);

        let mut spectrum = Self {
            reflectance,
            wavelengths,
            max_reflectance,
        };

        if normalize {
            let scale = CIE_Y_INTEGRAL / spectrum.inner_product(DenselySampledSpectrum::cie_y());
            for reflectance in &mut spectrum.reflectance {
                *reflectance *= scale;
            }
        }

        spectrum
    }

    pub fn swatch_reflectances() -> &'static [PiecewiseLinearSpectrum; 24] {
        SWATCH_REFLECTANCES_SPECTRUM.get_or_init(|| {
            SWATCH_REFLECTANCES
                .iter()
                .map(|swatch_reflectance| {
                    PiecewiseLinearSpectrum::from_interleaved(swatch_reflectance, false)
                })
                .collect::<Vec<PiecewiseLinearSpectrum>>()
                .try_into()
                .unwrap()
        })
    }

    pub fn cie_x() -> &'static PiecewiseLinearSpectrum {
        CIE_X_SPECTRUM_LPW
            .get_or_init(|| PiecewiseLinearSpectrum::new(CIE_X.to_vec(), CIE_LAMBDA.to_vec()))
    }

    pub fn cie_y() -> &'static PiecewiseLinearSpectrum {
        CIE_Y_SPECTRUM_LPW
            .get_or_init(|| PiecewiseLinearSpectrum::new(CIE_Y.to_vec(), CIE_LAMBDA.to_vec()))
    }

    pub fn cie_z() -> &'static PiecewiseLinearSpectrum {
        CIE_Z_SPECTRUM_LPW
            .get_or_init(|| PiecewiseLinearSpectrum::new(CIE_Z.to_vec(), CIE_LAMBDA.to_vec()))
    }

    pub fn cie_illum_d5000() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_D5000_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_D5000, true))
    }

    pub fn aces_illum_d60() -> &'static PiecewiseLinearSpectrum {
        ACES_ILLUM_D60_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(ACES_ILLUM_D60, true))
    }

    pub fn cie_illum_d6500() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_D6500_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_D6500, true))
    }

    pub fn cie_illum_f1() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F1_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F1, true))
    }

    pub fn cie_illum_f2() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F2_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F2, true))
    }

    pub fn cie_illum_f3() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F3_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F3, true))
    }

    pub fn cie_illum_f4() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F4_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F4, true))
    }

    pub fn cie_illum_f5() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F5_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F5, true))
    }

    pub fn cie_illum_f6() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F6_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F6, true))
    }

    pub fn cie_illum_f7() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F7_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F7, true))
    }

    pub fn cie_illum_f8() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F8_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F8, true))
    }

    pub fn cie_illum_f9() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F9_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F9, true))
    }

    pub fn cie_illum_f10() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F10_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F10, true))
    }

    pub fn cie_illum_f11() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F11_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F11, true))
    }

    pub fn cie_illum_f12() -> &'static PiecewiseLinearSpectrum {
        CIE_ILLUM_F12_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CIE_ILLUM_F12, true))
    }

    pub fn canon_eos_100d_r() -> &'static PiecewiseLinearSpectrum {
        CANON_EOS_100D_R_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CANON_EOS_100D_R, false))
    }

    pub fn canon_eos_100d_g() -> &'static PiecewiseLinearSpectrum {
        CANON_EOS_100D_G_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CANON_EOS_100D_G, false))
    }

    pub fn canon_eos_100d_b() -> &'static PiecewiseLinearSpectrum {
        CANON_EOS_100D_B_SPECTRUM
            .get_or_init(|| PiecewiseLinearSpectrum::from_interleaved(CANON_EOS_100D_B, false))
    }
}

impl Spectrum for PiecewiseLinearSpectrum {
    fn spectral_distribution(&self, lambda: f32) -> f32 {
        if self.wavelengths.is_empty()
            || lambda < *self.wavelengths.first().unwrap()
            || lambda > *self.wavelengths.last().unwrap()
        {
            0.0
        } else {
            let o = find_interval_fast(&self.wavelengths, lambda);

            let t =
                (lambda - self.wavelengths[o]) / (self.wavelengths[o + 1] - self.wavelengths[o]);
            self.reflectance[o].lerp(self.reflectance[o + 1], t)
        }
    }

    fn max_spectral_distribution(&self) -> f32 {
        self.max_reflectance
    }
}

static CIE_X_SPECTRUM: OnceLock<DenselySampledSpectrum> = OnceLock::new();
static CIE_Y_SPECTRUM: OnceLock<DenselySampledSpectrum> = OnceLock::new();
static CIE_Z_SPECTRUM: OnceLock<DenselySampledSpectrum> = OnceLock::new();

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

    pub fn cie_x() -> &'static DenselySampledSpectrum {
        CIE_X_SPECTRUM.get_or_init(|| {
            DenselySampledSpectrum::new_from_spectrum(
                PiecewiseLinearSpectrum::cie_x(),
                LAMBDA_MIN as u32,
                LAMBDA_MAX as u32,
            )
        })
    }

    pub fn cie_y() -> &'static DenselySampledSpectrum {
        CIE_Y_SPECTRUM.get_or_init(|| {
            DenselySampledSpectrum::new_from_spectrum(
                PiecewiseLinearSpectrum::cie_y(),
                LAMBDA_MIN as u32,
                LAMBDA_MAX as u32,
            )
        })
    }

    pub fn cie_z() -> &'static DenselySampledSpectrum {
        CIE_Z_SPECTRUM.get_or_init(|| {
            DenselySampledSpectrum::new_from_spectrum(
                PiecewiseLinearSpectrum::cie_z(),
                LAMBDA_MIN as u32,
                LAMBDA_MAX as u32,
            )
        })
    }

    pub fn cie_d(temperature: f32) -> DenselySampledSpectrum {
        let cct = temperature * 1.4388 / 1.4380;
        if cct < 4000.0 {
            let black_body_spectrum = BlackBodySpectrum::new(temperature);

            let mut reflectance = vec![];
            for lambda in LAMBDA_MIN as u32..=LAMBDA_MAX as u32 {
                reflectance.push(black_body_spectrum.spectral_distribution(lambda as f32));
            }

            DenselySampledSpectrum::new_from_spectral_distribution(
                reflectance,
                LAMBDA_MIN as u32,
                LAMBDA_MAX as u32,
            )
        } else {
            let x = if cct <= 7000.0 {
                -4.607 * 1e9 / cct.powi(3)
                    + 2.9678 * 1e6 / sqr(cct)
                    + 0.09911 * 1e3 / cct
                    + 0.244063
            } else {
                -2.0064 * 1e9 / cct.powi(3)
                    + 1.9018 * 1e6 / sqr(cct)
                    + 0.24748 * 1e3 / cct
                    + 0.23704
            };

            let y = -3.0 * x * x + 2.870 * x - 0.275;

            let m = 0.0241 + 0.2562 * x - 0.7341 * y;
            let m1 = (-1.3515 - 1.7703 * x + 5.9114 * y) / m;
            let m2 = (0.0300 - 31.4424 * x + 30.0717 * y) / m;

            let mut values = vec![0.0; CIE_SAMPLES];
            for i in 0..CIE_SAMPLES {
                values[i] = (CIE_S0[i] + CIE_S1[i] * m1 + CIE_S2[i] * m2) * 0.01;
            }

            let pwl = PiecewiseLinearSpectrum::new(values, CIE_S_LAMBDA.to_vec());
            DenselySampledSpectrum::new_from_spectrum(&pwl, LAMBDA_MIN as u32, LAMBDA_MAX as u32)
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
    illuminant: Arc<dyn Spectrum>,
}

impl RgbIlluminantSpectrum {
    pub fn new(rgb: Rgb, color_space: &RgbColorSpace, illuminant: Arc<dyn Spectrum>) -> Self {
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
