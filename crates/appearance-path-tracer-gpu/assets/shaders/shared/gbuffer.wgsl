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

// Source: https://jacco.ompf2.com/2024/01/18/reprojection-in-a-ray-tracer/
fn Frustum::point_ws_to_ss(_self: Frustum, ws: vec3<f32>, resolution: vec2<u32>, ss: ptr<function, vec2<f32>>) -> bool {
    let d_left: f32 = Plane::distance(_self.left, ws);
    let d_right: f32 = Plane::distance(_self.right, ws);
    let ss_x: f32 = (1.0 - d_left / (d_left + d_right)) * f32(resolution.x);
    if (ss_x < 0.0 || ss_x > f32(resolution.x - 1)) {
        return false;
    }

    let d_top: f32 = Plane::distance(_self.top, ws);
    let d_bottom: f32 = Plane::distance(_self.bottom, ws);
    let ss_y: f32 = (f32(resolution.y) * d_top) / (d_top + d_bottom);
    if (ss_y < 0.0 || ss_y > f32(resolution.y - 1)) {
        return false;
    }

    *ss = vec2<f32>(ss_x, ss_y);
    return true;
}