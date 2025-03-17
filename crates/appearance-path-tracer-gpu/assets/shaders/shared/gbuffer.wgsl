@include appearance-packing::shared/packing

struct GBufferTexel {
    position_ws: vec3<f32>,
    depth_ws: f32,
    normal_ws: PackedNormalizedXyz10,
    albedo: PackedRgb9e5,
    _padding0: u32,
    _padding1: u32,
}

fn GBufferTexel::new(position_ws: vec3<f32>, depth_ws: f32, normal_ws: vec3<f32>, albedo: vec3<f32>) -> GBufferTexel {
    return GBufferTexel(
        position_ws,
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        PackedRgb9e5::new(albedo),
        0,
        0
    );
}

fn GBufferTexel::depth_cs(_self: GBufferTexel, z_near: f32, z_far: f32) -> f32 {
    let z_linear: f32 = (_self.depth_ws - z_near) / z_far;
    return (z_near * z_far) / (z_far - z_linear * (z_far - z_near));
}

fn GBufferTexel::is_sky(_self: GBufferTexel) -> bool {
    return _self.depth_ws == 0.0;
}

fn GBufferTexel::geometric_similarity(_self: GBufferTexel, other: GBufferTexel) -> bool {
    let plane_dist: f32 = dot(PackedNormalizedXyz10::unpack(_self.normal_ws, 0), other.position_ws - _self.position_ws);
    return abs(plane_dist) <= 0.01 * _self.depth_ws;
}

struct Plane {
    p: vec4<f32>,
}

fn Plane::distance(_self: Plane, point: vec3<f32>) -> f32 {
    return dot(_self.p.xyz, point) - _self.p.w;
}

struct Frustum {
    left: Plane,
    right: Plane,
    top: Plane,
    bottom: Plane,
}
