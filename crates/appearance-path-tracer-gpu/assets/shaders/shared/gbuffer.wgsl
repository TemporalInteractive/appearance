@include appearance-packing::shared/packing

struct GBufferTexel {
    position_ws: vec3<f32>,
    depth_ws: f32,
    normal_ws: vec3<f32>,
    albedo: vec3<f32>,
}

struct PackedGBufferTexel {
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
        normal_ws,
        albedo
    );
}

fn PackedGBufferTexel::new(position_ws: vec3<f32>, depth_ws: f32, normal_ws: vec3<f32>, albedo: vec3<f32>) -> PackedGBufferTexel {
    return PackedGBufferTexel(
        position_ws,
        depth_ws,
        PackedNormalizedXyz10::new(normal_ws, 0),
        PackedRgb9e5::new(albedo),
        0,
        0
    );
}

fn PackedGBufferTexel::unpack(_self: PackedGBufferTexel) -> GBufferTexel {
    return GBufferTexel(
        _self.position_ws,
        _self.depth_ws,
        PackedNormalizedXyz10::unpack(_self.normal_ws, 0),
        PackedRgb9e5::unpack(_self.albedo)
    );
}

fn PackedGBufferTexel::depth_cs(_self: PackedGBufferTexel, z_near: f32, z_far: f32) -> f32 {
    let z_linear: f32 = (_self.depth_ws - z_near) / z_far;
    return (z_near * z_far) / (z_far - z_linear * (z_far - z_near));
}

fn PackedGBufferTexel::is_sky(_self: PackedGBufferTexel) -> bool {
    return _self.depth_ws == 0.0;
}

fn GBufferTexel::depth_cs(_self: GBufferTexel, z_near: f32, z_far: f32) -> f32 {
    let z_linear: f32 = (_self.depth_ws - z_near) / z_far;
    return (z_near * z_far) / (z_far - z_linear * (z_far - z_near));
}

fn GBufferTexel::is_sky(_self: GBufferTexel) -> bool {
    return _self.depth_ws == 0.0;
}