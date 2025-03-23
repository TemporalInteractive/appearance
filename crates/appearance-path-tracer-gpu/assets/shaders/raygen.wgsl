@include ::random
@include appearance-path-tracer-gpu::shared/ray

struct Constants {
    inv_view: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
    width: u32,
    height: u32,
    seed: u32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read_write> rays: array<Ray>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (id.x >= constants.width || id.y >= constants.height) { return; }
    let flat_id: u32 = id.y * constants.width + id.x;

    var rng: u32 = pcg_hash(flat_id ^ xor_shift_u32(constants.seed));

    let pixel_center = vec2<f32>(f32(id.x) + random_uniform_float(&rng), f32(id.y) + random_uniform_float(&rng));
    //let pixel_center = vec2<f32>(f32(id.x) + 0.5, f32(id.y) + 0.5);
    var uv: vec2<f32> = (pixel_center / vec2<f32>(f32(constants.width), f32(constants.height))) * 2.0 - 1.0;
    uv.y = -uv.y;
    let origin: vec4<f32> = constants.inv_view * vec4<f32>(0.0, 0.0, 0.0, 1.0);
    let targt: vec4<f32> = constants.inv_proj * vec4<f32>(uv, 1.0, 1.0);
    let direction: vec4<f32> = constants.inv_view * vec4<f32>(normalize(targt.xyz), 0.0);

    rays[id.y * constants.width + id.x] = Ray::new(origin.xyz, direction.xyz);
}