@include appearance-path-tracer-gpu::shared/gbuffer

struct GBufferConstants {
    prev_camera_frustum: Frustum,
}

@group(4)
@binding(0)
var<uniform> gbuffer_constants: GBufferConstants;

@group(4)
@binding(1)
var<storage, read_write> gbuffer: array<GBufferTexel>;

@group(4)
@binding(2)
var<storage, read> prev_gbuffer: array<GBufferTexel>;

fn GBuffer::reproject(point_ws: vec3<f32>, prev_point_ss: ptr<function, vec2<f32>>) -> bool {
    return Frustum::point_ws_to_ss(gbuffer_constants.prev_camera_frustum, point_ws, prev_point_ss);
}