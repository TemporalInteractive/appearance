pub mod spectrum;
pub use spectrum::*;

/// Speed of light (m/s)
pub const C: f32 = 299_792_458.0;

/// Planck's constant (J·s)
pub const H: f32 = 6.626_069_7e-34;

/// Boltzmann's constant (J/K)
pub const KB: f32 = 1.380_648_8e-23;

/// Computes emitted radiance at the given temperature in Kelvin for the given wavelength lambda.
pub fn black_body_emission(lambda: f32, t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }

    let l = lambda * 1e-9;
    let exponent = (H * C) / (l * KB * t);
    (2.0 * H * C * C) / (l.powf(5.0) * ((exponent.exp()) - 1.0))
}
