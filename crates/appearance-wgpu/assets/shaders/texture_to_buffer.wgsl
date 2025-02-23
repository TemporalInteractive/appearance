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
var<storage, read_write> buffer: array<u32>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (id.x >= constants.width || id.y >= constants.height) { return; }

    buffer[id.y * constants.width + id.x] = pack4x8unorm(textureLoad(texture, vec2(i32(id.x), i32(id.y))));
}