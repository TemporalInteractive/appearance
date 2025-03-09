@include ::packing

struct Ray {
    origin: vec3<f32>,
    direction: PackedNormalizedXyz10,
};

fn Ray::new(origin: vec3<f32>, direction: vec3<f32>) -> Ray {
    return Ray(origin, PackedNormalizedXyz10::new(direction, 0));
}

struct Payload {
    accumulated: PackedRgb9e5,
    throughput: PackedRgb9e5,
    rng: u32,
    t: f32,
};

fn Payload::new(accumulated: vec3<f32>, throughput: vec3<f32>, rng: u32, t: f32) -> Payload {
    return Payload(PackedRgb9e5::new(accumulated), PackedRgb9e5::new(throughput), rng, t);
}
