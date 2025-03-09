@include ::packing

struct GBufferTexel {
    depth_ws: f32,
    normal_ws: PackedNormalizedXyz10,
    _padding0: u32,
    _padding1: u32,
}

fn GBufferTexel::new(depth_ws: f32, normal_ws: vec3<f32>) -> GBufferTexel {
    return GBufferTexel(
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        0,
        0
    );
}

fn GBufferTexel::depth_cs(_self: GBufferTexel, z_near: f32, z_far: f32) -> f32 {
    let z_linear: f32 = (_self.depth_ws - z_near) / z_far;
    return (z_near * z_far) / (z_far - z_linear * (z_far - z_near));
}