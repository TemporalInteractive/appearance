struct Constants {
    width: u32,
    height: u32,
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
    
    textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(0.0, 0.0, f32(id.x) / 1000.0, 1.0));
}