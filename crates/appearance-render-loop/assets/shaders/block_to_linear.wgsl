@include appearance-render-loop::block

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
var texture: texture_storage_2d<rgba8unorm, read>;

@group(0)
@binding(2)
var out_texture: texture_storage_2d<rgba8unorm, read_write>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (id.x >= constants.width || id.y >= constants.height) { return; }
    let block_id: vec2<u32> = linear_to_block_pixel_idx(id, constants.width);

    let color: vec4<f32> = textureLoad(texture, vec2(i32(block_id.x), i32(block_id.y)));
    textureStore(out_texture, vec2(i32(id.x), i32(id.y)), color);
}