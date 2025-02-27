struct Ray {
    origin: vec3<f32>,
    _padding0: u32,
    direction: vec3<f32>,
    _padding1: u32,
};

fn Ray::new(origin: vec3<f32>, direction: vec3<f32>) -> Ray {
    return Ray(origin, 0, direction, 0);
}

struct Payload {
    accumulated: vec3<f32>,
    _padding0: u32,
};

fn Payload::new(accumulated: vec3<f32>) -> Payload {
    return Payload(accumulated, 0);
}