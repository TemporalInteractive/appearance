use std::{
    rc::Rc,
    sync::{Arc, OnceLock},
};

use glam::{FloatExt, Mat3, Vec2, Vec3};

use crate::math::{find_interval, lerp, sqr, Vec3Extensions};

use super::{
    data_tables::{
        self,
        cie::{CIE_ILLUM_D6500, CIE_X, CIE_Y, CIE_Z},
        rgb_color_space::{
            srgb_to_spectrum_coeffs, srgb_to_spectrum_scales, RgbSpectrumCoefficientArray,
        },
    },
    DenselySampledSpectrum, PiecewiseLinearSpectrum, SampledSpectrum, Spectrum, LAMBDA_MAX,
    LAMBDA_MIN,
};

pub const CIE_Y_INTEGRAL: f32 = 106.856895;

/// XYZ color space, a device-independent color space, which means that it does not describe the characteristics of a particular display or color measurement device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xyz(pub Vec3);

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
            DenselySampledSpectrum::cie_x().inner_product(spectrum),
            DenselySampledSpectrum::cie_y().inner_product(spectrum),
            DenselySampledSpectrum::cie_z().inner_product(spectrum),
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

    pub fn to_xy(self) -> Vec2 {
        let sum = self.0.element_sum();
        Vec2::new(self.0.x / sum, self.0.y / sum)
    }
}

/// RGB color space defined by using the chromaticities of red, green, and blue color primaries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub Vec3);

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

static SRGB_COLOR_SPACE: OnceLock<Arc<RgbColorSpace>> = OnceLock::new();

#[derive(Clone)]
pub struct RgbColorSpace {
    rgb_to_spectrum_table: RgbToSpectrumTable,
    illuminant: Arc<dyn Spectrum>,
    xyz_from_rgb: Mat3,
    rgb_from_xyz: Mat3,

    r: Vec2,
    g: Vec2,
    b: Vec2,
    w: Vec2,
}

impl RgbColorSpace {
    fn new(
        r_xy: Vec2,
        g_xy: Vec2,
        b_xy: Vec2,
        illuminant: Arc<dyn Spectrum>,
        rgb_to_spectrum_table: RgbToSpectrumTable,
    ) -> Self {
        let w = Xyz::from_spectrum(illuminant.as_ref());
        let w_xy = w.to_xy();

        let r = Xyz::from_xy(r_xy);
        let g = Xyz::from_xy(g_xy);
        let b = Xyz::from_xy(b_xy);
        let rgb = Mat3::from_cols_array(&[
            r.0.x, g.0.x, b.0.x, r.0.y, g.0.y, b.0.y, r.0.z, g.0.z, b.0.z,
        ])
        .transpose();

        let c = Xyz::new(rgb.inverse() * w.0);
        let xyz_from_rgb = rgb * Mat3::from_diagonal(c.0);
        let rgb_from_xyz = xyz_from_rgb.inverse();

        Self {
            rgb_to_spectrum_table,
            illuminant,
            xyz_from_rgb,
            rgb_from_xyz,
            r: r_xy,
            g: g_xy,
            b: b_xy,
            w: w_xy,
        }
    }

    pub fn srgb() -> Arc<Self> {
        SRGB_COLOR_SPACE
            .get_or_init(|| {
                // TODO: clean all them arcs everywhere
                let std_illum_65 = Arc::new(PiecewiseLinearSpectrum::cie_illum_d6500().clone());

                let srgb_spectrum_table =
                    RgbToSpectrumTable::new(srgb_to_spectrum_scales(), srgb_to_spectrum_coeffs());

                Arc::new(Self::new(
                    Vec2::new(0.64, 0.33),
                    Vec2::new(0.3, 0.6),
                    Vec2::new(0.15, 0.06),
                    std_illum_65,
                    srgb_spectrum_table,
                ))
            })
            .clone()
    }

    pub fn rgb_from_xyz_mat3(&self) -> &Mat3 {
        &self.rgb_from_xyz
    }

    pub fn xyz_to_rgb(&self, xyz: Xyz) -> Rgb {
        Rgb::new(self.rgb_from_xyz * xyz.0)
    }

    pub fn rgb_to_xyz(&self, rgb: Rgb) -> Xyz {
        Xyz::new(self.xyz_from_rgb * rgb.0)
    }

    pub fn rgb_to_polynomial(&self, rgb: Rgb) -> RgbSigmoidPolynomial {
        self.rgb_to_spectrum_table.rgb_to_polynomial(rgb)
    }

    pub fn illuminant(&self) -> &Arc<dyn Spectrum> {
        &self.illuminant
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RgbSigmoidPolynomial {
    c0: f32,
    c1: f32,
    c2: f32,
}

impl RgbSigmoidPolynomial {
    fn new(c0: f32, c1: f32, c2: f32) -> Self {
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

/// Retreives a RgbSigmoidPolynomial based on rgb values
#[derive(Clone)]
pub struct RgbToSpectrumTable {
    z_nodes: Arc<Box<[f32]>>,
    coefficients: Arc<RgbSpectrumCoefficientArray>,
}

impl RgbToSpectrumTable {
    pub const RESOLUTION: usize = 64;

    fn new(z_nodes: Arc<Box<[f32]>>, coefficients: Arc<RgbSpectrumCoefficientArray>) -> Self {
        Self {
            z_nodes,
            coefficients,
        }
    }

    pub fn rgb_to_polynomial(&self, rgb: Rgb) -> RgbSigmoidPolynomial {
        if rgb.0.x == rgb.0.y && rgb.0.y == rgb.0.z {
            RgbSigmoidPolynomial::new(
                0.0,
                0.0,
                (rgb.0.x - 0.5) / (rgb.0.x * (1.0 - rgb.0.x)).sqrt(),
            )
        } else {
            let i = rgb.0.max_element_idx();

            let z = rgb.0[i];
            let x = rgb.0[(i + 1) % 3] * (Self::RESOLUTION as f32 - 1.0) / z;
            let y = rgb.0[(i + 2) % 3] * (Self::RESOLUTION as f32 - 1.0) / z;

            let xi = (x as i32).min(Self::RESOLUTION as i32 - 2);
            let yi = (y as i32).min(Self::RESOLUTION as i32 - 2);
            let zi = find_interval(Self::RESOLUTION, |i| self.z_nodes[i] < z);

            let dx = x - xi as f32;
            let dy = y - yi as f32;
            let dz = (z - self.z_nodes[zi]) / (self.z_nodes[zi + 1] - self.z_nodes[zi]);

            let mut c = Vec3::ZERO;
            for j in 0..3 {
                let coefficient_lookup = |dx: usize, dy: usize, dz: usize| -> f32 {
                    self.coefficients[i][zi + dz][yi as usize + dy][xi as usize + dx][j]
                };

                // TODO: rewrite with glam lerp
                c[j] = lerp(
                    dz,
                    lerp(
                        dy,
                        lerp(dx, coefficient_lookup(0, 0, 0), coefficient_lookup(1, 0, 0)),
                        lerp(dx, coefficient_lookup(0, 1, 0), coefficient_lookup(1, 1, 0)),
                    ),
                    lerp(
                        dy,
                        lerp(dx, coefficient_lookup(0, 0, 1), coefficient_lookup(1, 0, 1)),
                        lerp(dx, coefficient_lookup(0, 1, 1), coefficient_lookup(1, 1, 1)),
                    ),
                );
            }

            RgbSigmoidPolynomial::new(c.x, c.y, c.z)
        }
    }
}
