@include ::math

struct Triangle {
    p0: vec3<f32>,
    p1: vec3<f32>,
    p2: vec3<f32>,
}

fn Triangle::new(p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>) -> Triangle {
    return Triangle(p0, p1, p2);
}

fn Triangle::empty() -> Triangle {
    return Triangle(vec3<f32>(0.0), vec3<f32>(0.0), vec3<f32>(0.0));
}

fn Triangle::area_from_edges(p01: vec3<f32>, p02: vec3<f32>) -> f32 {
    return length(cross(p01, p02)) * 0.5;
}

fn Triangle::solid_angle(cos_out: f32, area: f32, distance: f32) -> f32 {
    return min(TWO_PI, (cos_out * area) / (sqr(distance) + 0.0001));
}

fn Triangle::transform(_self: Triangle, transform: mat4x4<f32>) -> Triangle {
    return Triangle::new(
        (transform * vec4<f32>(_self.p0, 1.0)).xyz,
        (transform * vec4<f32>(_self.p1, 1.0)).xyz,
        (transform * vec4<f32>(_self.p2, 1.0)).xyz,
    );
}