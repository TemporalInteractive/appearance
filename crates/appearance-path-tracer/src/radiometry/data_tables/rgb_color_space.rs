use std::sync::{Arc, OnceLock};

pub type RgbSpectrumCoefficientArray = [[[[[f32; 3]; 64]; 64]; 64]; 3];

const SRGB_TO_SPECTRUM_SCALE_BYTES: &[u8] = include_bytes!("../../../acs/srgb.acss");
const SRGB_TO_SPECTRUM_COEFF_BYTES: &[u8] = include_bytes!("../../../acs/srgb.acsc");

static SRGB_TO_SPECTRUM_SCALE: OnceLock<Arc<Box<[f32]>>> = OnceLock::new();
static SRGB_TO_SPECTRUM_COEFF: OnceLock<Arc<RgbSpectrumCoefficientArray>> = OnceLock::new();

pub fn srgb_to_spectrum_scales() -> Arc<Box<[f32]>> {
    SRGB_TO_SPECTRUM_SCALE
        .get_or_init(|| Arc::new(bytemuck::cast_slice(SRGB_TO_SPECTRUM_SCALE_BYTES).into()))
        .clone()
}

pub fn srgb_to_spectrum_coeffs() -> Arc<RgbSpectrumCoefficientArray> {
    SRGB_TO_SPECTRUM_COEFF
        .get_or_init(|| {
            let flat_coeffs: &[f32] = bytemuck::cast_slice(SRGB_TO_SPECTRUM_COEFF_BYTES);
            Arc::new(unflatten_coeffs(flat_coeffs))
        })
        .clone()
}

fn unflatten_coeffs(flat_coeffs: &[f32]) -> RgbSpectrumCoefficientArray {
    let mut result = [[[[[0.0; 3]; 64]; 64]; 64]; 3];
    let mut idx = 0;

    #[allow(clippy::needless_range_loop)]
    for i in 0..3 {
        for j in 0..64 {
            for k in 0..64 {
                for l in 0..64 {
                    for m in 0..3 {
                        result[i][j][k][l][m] = flat_coeffs[idx];
                        idx += 1;
                    }
                }
            }
        }
    }
    result
}
