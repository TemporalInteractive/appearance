@include ::random
@include ::color
@include ::math

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
var<storage, read> demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read> in_temporal_demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(3)
var<storage, read_write> out_temporal_demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(4)
var<storage, read_write> temporal_frame_count: array<u32>;

@group(0)
@binding(5)
var velocity_texture: texture_storage_2d<rgba32float, read>;

fn bilinear(tx: f32, ty: f32, c00: vec3<f32>, c10: vec3<f32>, c01: vec3<f32>, c11: vec3<f32>) -> vec3<f32> {
    var a: vec3<f32> = c00 * (1.0 - tx) + c10 * tx;
    var b: vec3<f32> = c01 * (1.0 - tx) + c11 * tx;
    return a * (1.0 - ty) + b * ty;
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let current_gbuffer_texel: GBufferTexel = gbuffer[flat_id];

    if (GBufferTexel::is_sky(current_gbuffer_texel)) {
        return;
    }

    let radiance: vec3<f32> = PackedRgb9e5::unpack(demodulated_radiance[flat_id]);
    var temporal_radiance: vec3<f32> = radiance;
    var frame_count: u32 = 0;

    let velocity: vec2<f32> = textureLoad(velocity_texture, vec2<i32>(id)).xy;
    var prev_point_ss = vec2<f32>(id) - (vec2<f32>(constants.resolution) * velocity);
    var prev_id: u32;
    if (all(prev_point_ss >= vec2<f32>(0.0)) && all(prev_point_ss <= vec2<f32>(constants.resolution - 1))) {
        let prev_id_2d = vec2<u32>(floor(prev_point_ss));
        prev_id = prev_id_2d.y * constants.resolution.x + prev_id_2d.x;

        let prev_id00: u32 = min(u32(floor(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(floor(prev_point_ss.x)), constants.resolution.x - 1);
        let history00: vec3<f32> = PackedRgb9e5::unpack(in_temporal_demodulated_radiance[prev_id00]);
        let prev_id10: u32 = min(u32(ceil(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(floor(prev_point_ss.x)), constants.resolution.x - 1);
        let history10: vec3<f32> = PackedRgb9e5::unpack(in_temporal_demodulated_radiance[prev_id10]);
        let prev_id01: u32 = min(u32(floor(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(ceil(prev_point_ss.x)), constants.resolution.x - 1);
        let history01: vec3<f32> = PackedRgb9e5::unpack(in_temporal_demodulated_radiance[prev_id01]);
        let prev_id11: u32 = min(u32(ceil(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(ceil(prev_point_ss.x)), constants.resolution.x - 1);
        let history11: vec3<f32> = PackedRgb9e5::unpack(in_temporal_demodulated_radiance[prev_id11]);

        let reprojected_temporal_radiance: vec3<f32> = bilinear(fract(prev_point_ss.x), fract(prev_point_ss.y), history00, history10, history01, history11);

        let prev_gbuffer_texel: GBufferTexel = prev_gbuffer[prev_id];
        let current_depth_cs: f32 = GBufferTexel::depth_cs(current_gbuffer_texel, 0.001, 10000.0);
        let prev_depth_cs: f32 = GBufferTexel::depth_cs(prev_gbuffer_texel, 0.001, 10000.0);
        let valid_delta_depth: bool = (abs(current_depth_cs - prev_depth_cs) / current_depth_cs) < 0.1;
        let current_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(current_gbuffer_texel.normal_ws, 0);
        let prev_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(prev_gbuffer_texel.normal_ws, 0);
        let valid_delta_normal: bool = dot(current_normal_ws, prev_normal_ws) > 0.906; // 25 degrees

        if (valid_delta_depth && valid_delta_normal) {
            temporal_radiance = mix(radiance, reprojected_temporal_radiance, constants.history_influence);
            frame_count = temporal_frame_count[flat_id] + 1;
        }
    }



    // let prev_gbuffer_texel: GBufferTexel = prev_gbuffer[prev_id];
    // let current_depth_cs: f32 = GBufferTexel::depth_cs(current_gbuffer_texel, 0.001, 10000.0);
    // let prev_depth_cs: f32 = GBufferTexel::depth_cs(prev_gbuffer_texel, 0.001, 10000.0);
    // let valid_delta_depth: bool = (abs(current_depth_cs - prev_depth_cs) / current_depth_cs) < 0.1;
    // let current_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(current_gbuffer_texel.normal_ws, 0);
    // let prev_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(prev_gbuffer_texel.normal_ws, 0);
    // let valid_delta_normal: bool = dot(current_normal_ws, prev_normal_ws) > 0.906; // 25 degrees

    // if (valid_delta_depth && valid_delta_normal) {
    //     temporal_radiance = mix(radiance, temporal_radiance, constants.history_influence);
    // } else {
    //     temporal_radiance = radiance;
    // }

    out_temporal_demodulated_radiance[flat_id] = PackedRgb9e5::new(temporal_radiance);
    temporal_frame_count[flat_id] = frame_count;


    // var history_weight_factor: f32;
    // if (!valid_delta_depth) {
    //     history_weight_factor = 0.0;
    // } else {
    //     history_weight_factor = 1.0;
    // }
    // let blend_weight: f32 = 1.0 - (constants.history_influence * history_weight_factor);

    // let current_weight: f32 = saturate(blend_weight * (1.0 / (1.0 + linear_to_luma(reconstructed))));
    // let history_weight: f32 = saturate((1.0 - blend_weight) * (1.0 / (1.0 + linear_to_luma(clipped_history))));
    // reconstructed = (current_weight * reconstructed + history_weight * clipped_history) / (current_weight + history_weight);

    // demodulated_radiance[flat_id] = PackedRgb9e5::new(reconstructed);
}