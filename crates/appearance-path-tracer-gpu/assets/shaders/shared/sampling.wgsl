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