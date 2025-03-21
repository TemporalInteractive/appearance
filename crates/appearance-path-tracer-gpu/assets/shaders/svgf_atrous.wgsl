@include ::random
@include ::color
@include ::math

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

const PHI_NORMAL: f32 = 128.0;
const PHI_DEPTH: f32 = 0.1;
const PHI_LUMA: f32 = 4.0;

const KERNEL_WEIGHTS: array<f32, 3> = array<f32, 3>(
    3.0 / 8.0,
    1.0 / 4.0,
    1.0 / 16.0
);


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
var<storage, read> in_variance: array<f32>;

@group(0)
@binding(4)
var<storage, read_write> out_variance: array<f32>;

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
    var variance_sum: f32 = in_variance[flat_id];
    var weight_sum: f32 = 1.0;

    let current_luma: f32 = linear_to_luma(radiance_sum);
    let current_variance: f32 = variance_sum;

    // if (constants.pass_idx == 0) {
    //     out_history_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance_sum);
    // }

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

            let sample_radiance: vec3<f32> = PackedRgb9e5::unpack(in_temporal_demodulated_radiance[flat_sample_id]);
            let sample_luma: f32 = linear_to_luma(sample_radiance);

            let normal_weight: f32 = pow(max(dot(current_gbuffer_texel.normal_ws, sample_gbuffer_texel.normal_ws), 0.0), PHI_NORMAL);

            // This is slightly different compared to the SVGF paper, I didn't feel like calculating depth derivatives
            let depth_diff: f32 = abs(current_gbuffer_texel.depth_ws - sample_gbuffer_texel.depth_ws);
            let depth_weight: f32 = exp(-sqr(depth_diff) / (2.0 * sqr(PHI_DEPTH)));

            let luma_diff: f32 = abs(current_luma - sample_luma);
            let luma_weight: f32 = exp(-luma_diff / (max(PHI_LUMA * sqrt(current_variance), 1e-8)));

            var weight: f32 = normal_weight * depth_weight * luma_weight;
            weight *= KERNEL_WEIGHTS[abs(x)] * KERNEL_WEIGHTS[abs(y)];

            radiance_sum += sample_radiance * weight;
            variance_sum += in_variance[flat_sample_id] * sqr(weight);
            weight_sum += weight;
        }
    }

    radiance_sum /= weight_sum;
    variance_sum /= sqr(weight_sum);

    out_temporal_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance_sum);
    out_variance[flat_id] = variance_sum;
    if (constants.pass_idx == 0) {
        out_history_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance_sum);
    }
}