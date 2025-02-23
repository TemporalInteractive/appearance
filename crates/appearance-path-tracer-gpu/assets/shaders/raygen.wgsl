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
var texture: texture_storage_2d<rgba8unorm, write>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (id.x >= constants.width || id.y >= constants.height) { return; }

    var pixel_center = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    var uv: vec2<f32> = (pixel_center / vec2<f32>(f32(constants.width), f32(constants.height)));// * 2.0 - 1.0;
    
    let block_id = linear_to_block_pixel_idx(vec2(id.x, id.y), constants.width);
    textureStore(texture, vec2<i32>(block_id), vec4(uv.x, uv.y, 0.0, 1.0));
}