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
