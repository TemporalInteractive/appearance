@include appearance-render-loop::block
@include appearance-path-tracer-gpu::ray

struct Constants {
    width: u32,
    height: u32,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> payloads: array<Payload>;

@group(0)
@binding(2)
var texture: texture_storage_2d<rgba8unorm, read_write>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (id.x >= constants.width || id.y >= constants.height) { return; }
    var i: u32 = id.y * constants.width + id.x;

    var payload: Payload = payloads[i];
    var accumulated: vec3<f32> = payload.accumulated;

    //let block_id: vec2<u32> = linear_to_block_pixel_idx(id, constants.width);
    textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(accumulated.x, accumulated.y, accumulated.z, 1.0));
}