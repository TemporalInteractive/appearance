@include ::random
@include ::color

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    resolution: vec2<u32>,
    history_influence: f32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read_write> demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read> prev_demodulated_radiance: array<PackedRgb9e5>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let gbuffer_texel: GBufferTexel = gbuffer[flat_id];
    let prev_gbuffer_texel: GBufferTexel = prev_gbuffer[flat_id];

    var accumulated: vec3<f32> = PackedRgb9e5::unpack(demodulated_radiance[flat_id]);
    var prev_accumulated: vec3<f32> = PackedRgb9e5::unpack(prev_demodulated_radiance[flat_id]);

    accumulated = mix(accumulated, prev_accumulated, constants.history_influence);

    demodulated_radiance[flat_id] = PackedRgb9e5::new(accumulated);
}