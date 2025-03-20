@include ::random
@include ::color
@include ::math

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    resolution: vec2<u32>,
    pass_idx: u32,
    seed: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> in_temporal_demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read_write> out_temporal_demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(3)
var<storage, read> in_temporal_moments: array<vec2<f32>>;

@group(0)
@binding(4)
var<storage, read_write> out_temporal_moments: array<vec2<f32>>;

@group(0)
@binding(5)
var<storage, read_write> out_history_demodulated_radiance: array<PackedRgb9e5>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let current_gbuffer_texel: GBufferTexel = PackedGBufferTexel::unpack(gbuffer[flat_id]);

    if (GBufferTexel::is_sky(current_gbuffer_texel)) {
        return;
    }

    let step_size: u32 = 1u << constants.pass_idx;

    var radiance_sum: vec3<f32> = PackedRgb9e5::unpack(in_temporal_demodulated_radiance[flat_id]);
    var moments_sum: vec2<f32> = in_temporal_moments[flat_id];
    var weight_sum: f32 = 0.0;

    if (constants.pass_idx == 0) {
        out_history_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance_sum);
    }

    for (var y: i32 = -2; y <= 2; y += 1) {
        for (var x: i32 = -2; x <= 2; x += 1) {
            if (y == 0 && x == 0) {
                continue;
            }

            let sample_id: vec2<i32> = vec2<i32>(id) + vec2<i32>(x, y) * i32(step_size);
            if (any(sample_id < vec2<i32>(0)) || any(sample_id >= vec2<i32>(constants.resolution))) {
                continue;
            }
            let flat_sample_id: u32 = u32(sample_id.y) * constants.resolution.x + u32(sample_id.x);

            let sample_gbuffer_texel: GBufferTexel = PackedGBufferTexel::unpack(gbuffer[flat_sample_id]);
            if (GBufferTexel::is_sky(sample_gbuffer_texel)) {
                continue;
            }

            let weight: f32 = 1.0;

            radiance_sum += PackedRgb9e5::unpack(in_temporal_demodulated_radiance[flat_sample_id]) * weight;
            moments_sum += in_temporal_moments[flat_sample_id] * weight;
            weight_sum += weight;
        }
    }

    radiance_sum /= weight_sum;
    moments_sum /= weight_sum;

    out_temporal_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance_sum);
    out_temporal_moments[flat_id] = moments_sum;
    // if (constants.pass_idx == 0) {
    //     out_history_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance_sum);
    // }
}