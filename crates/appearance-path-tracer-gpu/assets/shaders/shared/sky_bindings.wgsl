@include ::math
@include appearance-path-tracer-gpu::shared/sampling

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

fn Sky::sun_intensity(zenith_angle_cos: f32) -> f32 {
    var cutoff_angle: f32 = PI / 1.95;
    var intensity: f32 = sky_constants.sun_intensity *
        max(0.0, 1.0 - exp(-((cutoff_angle - acos(zenith_angle_cos)) / 1.4)));
    return intensity;
}

fn Sky::direction_to_sun(uv: vec2<f32>) -> vec3<f32> {
    return normalize(perturb_direction_vector(uv, -sky_constants.sun_direction, sky_constants.sun_size * 0.1));
}

fn Sky::sun_solid_angle() -> f32 {
    return TWO_PI * (1.0 - cos(sky_constants.sun_size * 0.1));
}

fn Sky::sky(direction: vec3<f32>, skip_sun: bool) -> vec3<f32> {
    var sky_color = textureSampleLevel(sky_texture, sky_texture_sampler, unit_vector_to_panorama_coords(direction), 0.0).rgb;

    if (!skip_sun) {
        var l: vec3<f32> = -sky_constants.sun_direction;
        var intensity = Sky::sun_intensity(dot(l, UP));

        var cos_theta: f32 = dot(direction, l);
        var sun_angular_diameter_cos: f32 = cos(sky_constants.sun_size * 0.1);
        var sundisk: f32 = smoothstep(sun_angular_diameter_cos, sun_angular_diameter_cos + 0.4, cos_theta);
        //select(0.0, 1.0, cos_theta > sun_angular_diameter_cos);

        sky_color += intensity * 1000.0 * sundisk * sky_constants.sun_color;
    }

    return sky_color;
}