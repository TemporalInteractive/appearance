@include ::random
@include ::color

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    resolution: vec2<u32>,
    remodulate: u32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read_write> demodulated_radiance: array<PackedRgb9e5>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let albedo: vec3<f32> = PackedRgb9e5::unpack(gbuffer[flat_id].albedo);

    var radiance: vec3<f32> = PackedRgb9e5::unpack(radiance[flat_id]);
    if (constants.remodulate > 0) {
        radiance *= albedo + 0.0001;
    } else {
        radiance /= albedo + 0.0001;
    }

    demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance);
}