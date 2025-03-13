const PI: f32 = 3.141592653589793238462643383279502884197169;
const INV_PI: f32 = (1.0 / PI);
const INV_2_PI: f32 = (1.0 / (2.0 * PI));
const INV_4_PI: f32 = (1.0 / (4.0 * PI));
const TWO_PI: f32 = 2.0 * PI;
const HALF_PI: f32 = 0.5 * PI;

const F32_MIN: f32 = -3.40282347E+38;
const F32_MAX: f32 = 3.40282347E+38;
const U32_MAX: u32 = 4294967295u;
const U16_MAX: u32 = 65535u;

const GOLDEN_RATIO: f32 = 1.6180339887498948482;
const GOLDEN_ANGLE: f32 = 2.39996322972865332;

const RIGHT: vec3<f32> = vec3<f32>(1.0, 0.0, 0.0);
const UP: vec3<f32> = vec3<f32>(0.0, 1.0, 0.0);
const FORWARD: vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);

fn safe_sqrt(x: f32) -> f32 {
    return sqrt(max(0.0, x));
}

fn sqr(x: f32) -> f32 {
    return x * x;
}

fn is_nan(x: f32) -> bool {
    return x != x;
}

fn check_and_fix_nan(x: f32) -> f32 {
    if (is_nan(x)) {
        return 0.0;
    }
    return x;
}

fn check_and_fix_nan3(x: vec3<f32>) -> vec3<f32> {
    if (is_nan(x.x) || is_nan(x.y) || is_nan(x.z)) {
        return vec3<f32>(0.0);
    }
    return x;
}

fn clamp_intensity(x: vec3<f32>, max_intensity: f32) -> vec3<f32> {
    let intensity: f32 = max(x.x, max(x.y, x.z));
    if (intensity > max_intensity) {
        return x * (max_intensity / intensity);
    }
    return x;
}

// http://jcgt.org/published/0006/01/01/
fn build_orthonormal_basis(n: vec3<f32>) -> mat3x3<f32> {
    var t1: vec3<f32>;
    var t2: vec3<f32>;
    if (n.z < 0.0) {
        let a: f32 = 1.0 / (1.0 - n.z);
        let b: f32 = n.x * n.y * a;
        t1 = vec3<f32>(1.0 - n.x * n.x * a, -b, n.x);
        t2 = vec3<f32>(b, n.y * n.y * a - 1.0, -n.y);
    } else {
        let a: f32 = 1.0 / (1.0 + n.z);
        let b: f32 = -n.x * n.y * a;
        t1 = vec3<f32>(1.0 - n.x * n.x * a, b, -n.x);
        t2 = vec3<f32>(b, 1.0 - n.y * n.y * a, -n.y);
    }
    return mat3x3<f32>(t1, t2, n);
}

// (from "Efficient Construction of Perpendicular Vectors Without Branching", 2009)
fn get_perpendicular_vector(u: vec3<f32>) -> vec3<f32> {
    let a: vec3<f32> = abs(u);

    // Be explicit about uint types in the ternary to work around
    // https://github.com/microsoft/DirectXShaderCompiler/issues/4727
    let xm: u32 = select(0u, 1u, ((a.x - a.y) < 0 && (a.x - a.z) < 0));
    let ym: u32 = select(0u, 1 ^ xm, (a.y - a.z) < 0);
    let zm: u32 = 1 ^ (xm | ym);

    return cross(u, vec3<f32>(f32(xm), f32(ym), f32(zm)));
}

fn unit_vector_to_panorama_coords(direction: vec3<f32>) -> vec2<f32> {
    let phi: f32 = atan2(direction.z, direction.x) + PI;
    let theta: f32 = acos(direction.y);

    return vec2<f32>(phi / (2.0 * PI), theta * INV_PI);
}