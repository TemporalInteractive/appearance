@include ::math

fn get_cosine_hemisphere_sample(uv: vec2<f32>) -> vec3<f32> {
    var phi: f32 = TWO_PI * uv.x;
    var sin_theta: f32 = sqrt(1.0 - uv.y);
    var sin_phi: f32 = sin(phi);
    var cos_phi: f32 = cos(phi);

    return vec3<f32>(
        sin_phi * sin_theta,
        cos_phi * sin_theta,
        safe_sqrt(uv.y)
    );
}

fn get_uniform_hemisphere_sample(uv: vec2<f32>) -> vec3<f32> {
    var phi: f32 = TWO_PI * uv.x;
    var r: f32 = sqrt(1.0 - uv.y * uv.y);
    var sin_phi: f32 = sin(phi);
    var cos_phi: f32 = cos(phi);

    return vec3<f32>(
        r * cos_phi,
        r * sin_phi,
        uv.y
    );
}

fn get_uniform_sphere_sample(uv: vec2<f32>) -> vec3<f32> {
    var phi: f32 = TWO_PI * uv.x;
    var theta: f32 = acos(1.0 - 2.0 * uv.y);

    var sin_phi: f32 = sin(phi);
    var cos_phi: f32 = cos(phi);
    var sin_theta: f32 = sin(theta);
    var cos_theta: f32 = cos(theta);

    return vec3<f32>(
        sin_theta * cos_phi,
        sin_theta * sin_phi,
        cos_theta
    );
}

// from https://stackoverflow.com/a/2660181
fn perturb_direction_vector(uv: vec2<f32>, direction: vec3<f32>, angle: f32) -> vec3<f32> {
    let h: f32 = cos(angle);

    let phi: f32 = 2.0 * PI * uv.x;

    let z: f32 = h + (1.0 - h) * uv.y;
    let sin_t: f32 = sqrt(1.0 - z * z);

    let x: f32 = cos(phi) * sin_t;
    let y: f32 = sin(phi) * sin_t;

    let bitangent: vec3<f32> = get_perpendicular_vector(direction);
    let tangent: vec3<f32> = cross(bitangent, direction);

    return bitangent * x + tangent * y + direction * z;
}

// https://www.iryoku.com/next-generation-post-processing-in-call-of-duty-advanced-warfare/
fn interleaved_gradient_noise(pos: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(0.06711056 * pos.x + 0.00583715 * pos.y));
}

// https://blog.demofox.org/2022/01/01/interleaved-gradient-noise-a-different-kind-of-low-discrepancy-sequence/
fn interleaved_gradient_noise_animated(pos: vec2<u32>, frame: u32) -> f32 {
    let id: u32 = frame % 64;
    let x: f32 = f32(pos.x) + 5.588238 * f32(id);
    let y: f32 = f32(pos.y) + 5.588238 * f32(id);
    return interleaved_gradient_noise(vec2(x, y));
}