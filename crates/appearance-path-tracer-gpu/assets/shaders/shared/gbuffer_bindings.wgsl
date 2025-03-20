@include appearance-path-tracer-gpu::shared/gbuffer

struct GBufferConstants {
    prev_camera_frustum: Frustum,
    resolution: vec2<u32>,
    // prev_world_to_view: mat4x4<f32>,
    // prev_view_to_screen: mat4x4<f32>,
}

@group(4)
@binding(0)
var<uniform> gbuffer_constants: GBufferConstants;

@group(4)
@binding(1)
var<storage, read_write> gbuffer: array<PackedGBufferTexel>;

@group(4)
@binding(2)
var<storage, read> prev_gbuffer: array<PackedGBufferTexel>;

fn GBuffer::sample_prev_gbuffer(pos: vec2<f32>) -> GBufferTexel {
    let i_pos = vec2<u32>(floor(pos));
    let f_pos: vec2<f32> = fract(pos);

    let idx00: u32 = i_pos.y * gbuffer_constants.resolution.x + i_pos.x;
    let idx10: u32 = i_pos.y * gbuffer_constants.resolution.x + min(i_pos.x + 1, gbuffer_constants.resolution.x - 1);
    let idx01: u32 = min(i_pos.y + 1, gbuffer_constants.resolution.y - 1) * gbuffer_constants.resolution.x + i_pos.x;
    let idx11: u32 = min(i_pos.y + 1, gbuffer_constants.resolution.y - 1) * gbuffer_constants.resolution.x + min(i_pos.x + 1, gbuffer_constants.resolution.x - 1);

    let t00: GBufferTexel = PackedGBufferTexel::unpack(prev_gbuffer[idx00]);
    let t10: GBufferTexel = PackedGBufferTexel::unpack(prev_gbuffer[idx10]);
    let t01: GBufferTexel = PackedGBufferTexel::unpack(prev_gbuffer[idx01]);
    let t11: GBufferTexel = PackedGBufferTexel::unpack(prev_gbuffer[idx11]);

    let position_ws_0: vec3<f32> = mix(t00.position_ws, t10.position_ws, f_pos.x);
    let position_ws_1: vec3<f32> = mix(t01.position_ws, t11.position_ws, f_pos.x);
    let position_ws: vec3<f32> = mix(position_ws_0, position_ws_1, f_pos.y);

    let depth_ws_0: f32 = mix(t00.depth_ws, t10.depth_ws, f_pos.x);
    let depth_ws_1: f32 = mix(t01.depth_ws, t11.depth_ws, f_pos.x);
    let depth_ws: f32 = mix(depth_ws_0, depth_ws_1, f_pos.y);

    let normal_ws_0: vec3<f32> = mix(t00.normal_ws, t10.normal_ws, f_pos.x);
    let normal_ws_1: vec3<f32> = mix(t01.normal_ws, t11.normal_ws, f_pos.x);
    let normal_ws: vec3<f32> = normalize(mix(normal_ws_0, normal_ws_1, f_pos.y));

    let albedo_0: vec3<f32> = mix(t00.albedo, t10.albedo, f_pos.x);
    let albedo_1: vec3<f32> = mix(t01.albedo, t11.albedo, f_pos.x);
    let albedo: vec3<f32> = mix(albedo_0, albedo_1, f_pos.y);

    return GBufferTexel::new(position_ws, depth_ws, normal_ws, albedo);
}

fn GBufferTexel::is_disoccluded(_self: GBufferTexel, prev_bilinear: GBufferTexel) -> bool {
    let depth_similar: bool = abs(_self.depth_ws - prev_bilinear.depth_ws) <= 0.1 * prev_bilinear.depth_ws;
    let normal_similar: bool = dot(_self.normal_ws, prev_bilinear.normal_ws) > 0.906; // 25 degrees
    return depth_similar && normal_similar;
}

// fn GBufferTexel::disoccluded(_self: GBufferTexel, prev_bilinear: GBufferTexel, prev_depth_ws: vec4<f32>, threshold: f32) -> vec4<bool> {
//     let no_x_prev1: f32 = abs(dot(prev_bilinear.position_ws, _self.normal_ws));
//     let no_x_prev2: f32 = abs(dot(prev_bilinear.position_ws, prev_bilinear.normal_ws));
//     let no_x_prev: f32 = max(no_x_prev1, no_x_prev2) / _self.depth_ws;
//     let z_prev: f32 = prev_bilinear.depth_ws;
//     let no_v_prev: f32 = no_x_prev / abs(z_prev);
//     let relative_plane_dist: vec4<f32> = abs(vec4<f32>(no_v_prev) * abs(prev_depth_ws) - vec4<f32>(no_x_prev));

//     return relative_plane_dist < vec4<f32>(threshold);
// }