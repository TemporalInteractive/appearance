@include appearance-packing::shared/packing

struct GBufferTexel {
    normal_ws: vec3<f32>,
    depth_ws: f32,
};

fn GBufferTexel::new(texel: vec4<f32>) -> GBufferTexel {
    return GBufferTexel(
        texel.gba,
        texel.r
    );
}

fn GBufferTexel::to_texel(_self: GBufferTexel) -> vec4<f32> {
    return vec4<f32>(
        _self.depth_ws,
        _self.normal_ws
    );
}