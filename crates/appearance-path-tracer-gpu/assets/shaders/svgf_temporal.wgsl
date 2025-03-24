@include ::random
@include ::color
@include ::math

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    resolution: vec2<u32>,
    max_history_frames: u32,
    seed: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read> history_demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(3)
var<storage, read_write> out_temporal_demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(4)
var<storage, read> in_temporal_moments: array<vec2<f32>>;

@group(0)
@binding(5)
var<storage, read_write> out_temporal_moments: array<vec2<f32>>;

@group(0)
@binding(6)
var<storage, read_write> out_variance: array<f32>;

@group(0)
@binding(7)
var<storage, read_write> temporal_frame_count: array<u32>;

@group(0)
@binding(8)
var velocity_texture: texture_storage_2d<rgba32float, read>;

fn sample_temporal_demodulated_radiance(pos: vec2<f32>) -> vec3<f32> {
    let i_pos = vec2<u32>(floor(pos));
    let f_pos: vec2<f32> = fract(pos);

    let idx00: u32 = i_pos.y * constants.resolution.x + i_pos.x;
    let idx10: u32 = i_pos.y * constants.resolution.x + min(i_pos.x + 1, constants.resolution.x - 1);
    let idx01: u32 = min(i_pos.y + 1, constants.resolution.y - 1) * constants.resolution.x + i_pos.x;
    let idx11: u32 = min(i_pos.y + 1, constants.resolution.y - 1) * constants.resolution.x + min(i_pos.x + 1, constants.resolution.x - 1);

    let c00: vec3<f32> = PackedRgb9e5::unpack(history_demodulated_radiance[idx00]);
    let c10: vec3<f32> = PackedRgb9e5::unpack(history_demodulated_radiance[idx10]);
    let c01: vec3<f32> = PackedRgb9e5::unpack(history_demodulated_radiance[idx01]);
    let c11: vec3<f32> = PackedRgb9e5::unpack(history_demodulated_radiance[idx11]);

    let c0: vec3<f32> = mix(c00, c10, f_pos.x);
    let c1: vec3<f32> = mix(c01, c11, f_pos.x);
    return mix(c0, c1, f_pos.y);
}

fn sample_temporal_moments(pos: vec2<f32>) -> vec2<f32> {
    let i_pos = vec2<u32>(floor(pos));
    let f_pos: vec2<f32> = fract(pos);

    let idx00: u32 = i_pos.y * constants.resolution.x + i_pos.x;
    let idx10: u32 = i_pos.y * constants.resolution.x + min(i_pos.x + 1, constants.resolution.x - 1);
    let idx01: u32 = min(i_pos.y + 1, constants.resolution.y - 1) * constants.resolution.x + i_pos.x;
    let idx11: u32 = min(i_pos.y + 1, constants.resolution.y - 1) * constants.resolution.x + min(i_pos.x + 1, constants.resolution.x - 1);

    let c00: vec2<f32> = in_temporal_moments[idx00];
    let c10: vec2<f32> = in_temporal_moments[idx10];
    let c01: vec2<f32> = in_temporal_moments[idx01];
    let c11: vec2<f32> = in_temporal_moments[idx11];

    let c0: vec2<f32> = mix(c00, c10, f_pos.x);
    let c1: vec2<f32> = mix(c01, c11, f_pos.x);
    return mix(c0, c1, f_pos.y);
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let current_gbuffer_texel: GBufferTexel = PackedGBufferTexel::unpack(gbuffer[flat_id]);

    let radiance: vec3<f32> = PackedRgb9e5::unpack(demodulated_radiance[flat_id]);

    if (GBufferTexel::is_sky(current_gbuffer_texel)) {
        out_temporal_demodulated_radiance[flat_id] = PackedRgb9e5::new(radiance);
        return;
    }

    var rng: u32 = pcg_hash(flat_id ^ xor_shift_u32(constants.seed));

    var moments: vec2<f32>;
    moments.x = linear_to_luma(radiance);
    moments.y = sqr(moments.x);

    var temporal_radiance = vec3<f32>(0.0);
    var temporal_moments = vec2<f32>(0.0);
    var frame_count: u32 = 0;

    let velocity: vec2<f32> = textureLoad(velocity_texture, vec2<i32>(id)).xy;
    var prev_point_ss = vec2<f32>(id) - (vec2<f32>(constants.resolution) * velocity);
    var prev_id: u32;
    if (all(prev_point_ss >= vec2<f32>(0.0)) && all(prev_point_ss <= vec2<f32>(constants.resolution - 1))) {
        let prev_id_2d = vec2<u32>(floor(prev_point_ss));
        prev_id = prev_id_2d.y * constants.resolution.x + prev_id_2d.x;

        let prev_gbuffer_texel: GBufferTexel = GBuffer::sample_prev_gbuffer(prev_point_ss);
        if (GBufferTexel::is_disoccluded(current_gbuffer_texel, prev_gbuffer_texel)) {
            temporal_radiance = sample_temporal_demodulated_radiance(prev_point_ss);
            temporal_moments = sample_temporal_moments(prev_point_ss);

            frame_count = min(temporal_frame_count[flat_id] + 1, constants.max_history_frames);
        }
    }

    let alpha: f32 = 1.0 / (1.0 + f32(frame_count));
    temporal_moments = mix(temporal_moments, moments, alpha);
    temporal_radiance = mix(temporal_radiance, radiance, alpha);

    out_variance[flat_id] = max(temporal_moments.y - sqr(temporal_moments.x), 0.0);

    out_temporal_demodulated_radiance[flat_id] = PackedRgb9e5::new(temporal_radiance);
    out_temporal_moments[flat_id] = temporal_moments;
    temporal_frame_count[flat_id] = frame_count;
}