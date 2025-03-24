@include ::math
@include appearance-path-tracer-gpu::shared/sampling

const SUN_DISTANCE: f32 = 1e+6;

struct SkyConstants {
    sun_direction: vec3<f32>,
    sun_size: f32,
    sun_color: vec3<f32>,   
    sun_intensity: f32,
}

@group(3)
@binding(0)
var<uniform> sky_constants: SkyConstants;

@group(3)
@binding(1)
var sky_texture: texture_2d<f32>;

@group(3)
@binding(2)
var sky_texture_sampler: sampler;

fn Sky::sun_intensity(direction: vec3<f32>) -> f32 {
    const CUTOFF_ANGLE: f32 = PI / 1.95;

    let l: vec3<f32> = -sky_constants.sun_direction;
    let zenith_angle_cos: f32 = dot(l, UP);
    var intensity: f32 = sky_constants.sun_intensity *
        max(0.0, 1.0 - exp(-((CUTOFF_ANGLE - acos(zenith_angle_cos)) / 1.4)));

    let cos_theta: f32 = dot(direction, l);
    let sun_angular_diameter_cos: f32 = cos(sky_constants.sun_size * 0.1);
    intensity *= select(0.0, 1.0, cos_theta > sun_angular_diameter_cos);

    return intensity;
}

fn Sky::direction_to_sun(uv: vec2<f32>) -> vec3<f32> {
    return normalize(perturb_direction_vector(uv, -sky_constants.sun_direction, sky_constants.sun_size * 0.1));
}

fn Sky::inverse_direction_to_sun(direction: vec3<f32>) -> vec2<f32> {
    let sun_dir: vec3<f32> = -sky_constants.sun_direction;
    let bitangent: vec3<f32> = get_perpendicular_vector(sun_dir);
    let tangent: vec3<f32> = cross(bitangent, sun_dir);

    // Project direction onto basis vectors
    let z: f32 = dot(direction, sun_dir);
    let x: f32 = dot(direction, bitangent);
    let y: f32 = dot(direction, tangent);

    // Compute uv coordinates
    let sin_t: f32 = sqrt(1.0 - z * z);
    let phi: f32 = atan2(y, x);
    
    let uv_x: f32 = (phi / (2.0 * PI)) % 1.0; // Normalize phi to [0,1]
    let uv_y: f32 = (z - cos(sky_constants.sun_size * 0.1)) / (1.0 - cos(sky_constants.sun_size * 0.1)); 

    return vec2<f32>(uv_x, uv_y);
}

fn Sky::sun_solid_angle() -> f32 {
    return TWO_PI * (1.0 - cos(sky_constants.sun_size * 0.1));
}

fn Sky::sky(direction: vec3<f32>, skip_sun: bool) -> vec3<f32> {
    var sky_color = textureSampleLevel(sky_texture, sky_texture_sampler, unit_vector_to_panorama_coords(direction), 0.0).rgb;

    if (!skip_sun) {
        var intensity = Sky::sun_intensity(direction);

        sky_color += intensity * 1000.0 * sky_constants.sun_color;
    }

    return sky_color;
}