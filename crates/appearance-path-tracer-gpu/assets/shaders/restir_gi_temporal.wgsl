@include ::random
@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/gbuffer
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir/gi_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings
@include appearance-path-tracer-gpu::shared/gbuffer_bindings

@include appearance-path-tracer-gpu::helpers/nee
@include appearance-path-tracer-gpu::helpers/trace
@include appearance-path-tracer-gpu::helpers/inline_path_tracer

struct Constants {
    resolution: vec2<u32>,
    ray_count: u32,
    spatial_pass_count: u32,
    unbiased: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> in_rays: array<Ray>;

@group(0)
@binding(2)
var<storage, read_write> payloads: array<Payload>;

@group(0)
@binding(3)
var scene: acceleration_structure;

@group(0)
@binding(4)
var<storage, read> reservoirs_in: array<PackedGiReservoir>;

@group(0)
@binding(5)
var<storage, read_write> reservoirs_out: array<PackedGiReservoir>;

@group(0)
@binding(6)
var<storage, read> prev_reservoirs_in: array<PackedGiReservoir>;

@group(0)
@binding(7)
var<storage, read_write> prev_reservoirs_out: array<PackedGiReservoir>;

@group(0)
@binding(8)
var<storage, read> light_sample_ctxs: array<LightSampleCtx>;

@group(0)
@binding(9)
var velocity_texture: texture_storage_2d<rgba32float, read>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let ray: Ray = in_rays[flat_id];
    var origin: vec3<f32> = ray.origin;
    var direction: vec3<f32> = PackedNormalizedXyz10::unpack(ray.direction, 0);

    var payload: Payload = payloads[flat_id];
    if (payload.t < 0.0) { return; } // TODO: indirect dispatch with pids
    
    var rng: u32 = payload.rng;
    let throughput: vec3<f32> = PackedRgb9e5::unpack(payload.throughput);
    let hit_point_ws = origin + direction * payload.t;

    let light_sample_ctx: LightSampleCtx = light_sample_ctxs[flat_id];
    var reservoir: GiReservoir = PackedGiReservoir::unpack(reservoirs_in[flat_id]);

    let velocity: vec2<f32> = textureLoad(velocity_texture, vec2<i32>(id)).xy;
    var prev_point_ss = vec2<f32>(id) - (vec2<f32>(constants.resolution) * velocity);
    if (all(prev_point_ss >= vec2<f32>(0.0)) && all(prev_point_ss <= vec2<f32>(constants.resolution - 1))) {
        let prev_id_2d = vec2<u32>(floor(prev_point_ss));
        let prev_id: u32 = prev_id_2d.y * constants.resolution.x + prev_id_2d.x;

        let current_gbuffer_texel: GBufferTexel = PackedGBufferTexel::unpack(gbuffer[flat_id]);
        let prev_gbuffer_texel: GBufferTexel = GBuffer::sample_prev_gbuffer(prev_point_ss);
        
        if (GBufferTexel::is_disoccluded(current_gbuffer_texel, prev_gbuffer_texel)) {
            var prev_reservoir: GiReservoir = PackedGiReservoir::unpack(prev_reservoirs_in[prev_id]);
            prev_reservoir.sample_count = min(prev_reservoir.sample_count, 30.0);

            let w_out_worldspace: vec3<f32> = -direction;
            prev_reservoir.selected_phat = GiReservoir::phat(prev_reservoir, light_sample_ctx, throughput, hit_point_ws, w_out_worldspace, scene);

            reservoir = GiReservoir::combine(reservoir, prev_reservoir, &rng);
        }
    }
    
    reservoirs_out[flat_id] = PackedGiReservoir::new(reservoir);
    if (constants.spatial_pass_count == 0) {
        prev_reservoirs_out[flat_id] = PackedGiReservoir::new(reservoir);
    }

    payload.rng = rng;
    payloads[flat_id] = payload;
}