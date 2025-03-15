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

@compute
@workgroup_size(128)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: u32 = global_id.x;
    if (id >= constants.ray_count) { return; }

    let ray: Ray = in_rays[id];
    var origin: vec3<f32> = ray.origin;
    var direction: vec3<f32> = PackedNormalizedXyz10::unpack(ray.direction, 0);

    var payload: Payload = payloads[id];
    if (payload.t < 0.0) { return; } // TODO: indirect dispatch with pids
    
    let light_sample_ctx: LightSampleCtx = light_sample_ctxs[id];
    
    var rng: u32 = payload.rng;
    let throughput: vec3<f32> = PackedRgb9e5::unpack(payload.throughput);

    let tex_coord: vec2<f32> = light_sample_ctx.hit_tex_coord;
    let material_idx: u32 = light_sample_ctx.hit_material_idx;
    let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
    let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord);
    let disney_bsdf = DisneyBsdf::from_material(material);

    let hit_point_ws = origin + direction * payload.t;
    let front_facing_shading_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(light_sample_ctx.front_facing_shading_normal_ws, 0);
    let tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_shading_normal_ws);
    let world_to_tangent: mat3x3<f32> = transpose(tangent_to_world);

    let front_facing_clearcoat_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(light_sample_ctx.front_facing_clearcoat_normal_ws, 0);
    let clearcoat_tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_clearcoat_normal_ws);
    let clearcoat_world_to_tangent: mat3x3<f32> = transpose(clearcoat_tangent_to_world);

    let reservoir: GiReservoir = PackedGiReservoir::unpack(reservoirs_in[id]);

    var prev_point_ss: vec2<f32>;
    var prev_id: u32;
    if (GBuffer::reproject(hit_point_ws, constants.resolution, &prev_point_ss)) {
        // prev_point_ss += random_uniform_float2(&rng) * 0.5;
        let prev_id_2d = vec2<u32>(floor(prev_point_ss));
        prev_id = prev_id_2d.y * constants.resolution.x + prev_id_2d.x;
    } else {
        prev_id = id;
    }

    var valid_prev_reservoir: bool = true;
    let current_gbuffer_texel: GBufferTexel = gbuffer[id];
    let prev_gbuffer_texel: GBufferTexel = prev_gbuffer[prev_id];
    let prev_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(prev_gbuffer_texel.normal_ws, 0);
    if (constants.unbiased == 0) {
        let current_depth_cs: f32 = GBufferTexel::depth_cs(current_gbuffer_texel, 0.001, 10000.0);
        let prev_depth_cs: f32 = GBufferTexel::depth_cs(prev_gbuffer_texel, 0.001, 10000.0);
        let valid_delta_depth: bool = (abs(current_depth_cs - prev_depth_cs) / current_depth_cs) < 0.1;
        let current_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(current_gbuffer_texel.normal_ws, 0);
        let valid_delta_normal: bool = dot(current_normal_ws, prev_normal_ws) > 0.906; // 25 degrees

        valid_prev_reservoir = valid_delta_depth && valid_delta_normal;
    }

    if (valid_prev_reservoir) {
        var prev_reservoir: GiReservoir = PackedGiReservoir::unpack(prev_reservoirs_in[prev_id]);
        prev_reservoir.sample_count = min(prev_reservoir.sample_count, 20.0 * reservoir.sample_count);

        let w_out_worldspace: vec3<f32> = -direction;
        let w_in_worldspace: vec3<f32> = prev_reservoir.w_in_worldspace;

        var shading_pdf: f32;
        let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
            tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
            w_out_worldspace, w_in_worldspace, &shading_pdf);
        var cos_in: f32 = abs(dot(w_in_worldspace, front_facing_shading_normal_ws));
        //cos_in *= jacobianDiffuse(current_gbuffer_texel.position_ws, prev_gbuffer_texel.position_ws, prev_normal_ws, w_in_worldspace, payload.t);

        let local_throughput: vec3<f32> = cos_in * reflectance;
        let gi_origin: vec3<f32> = hit_point_ws + w_in_worldspace * 0.0001;
        let gi_direction: vec3<f32> = w_in_worldspace;
        var throughput_result: vec3<f32> = throughput;
        let contribution: vec3<f32> = throughput * local_throughput * InlinePathTracer::trace(gi_origin, gi_direction, RESTIR_GI_PHAT_MAX_BOUNCES, &throughput_result, &rng, scene);
        prev_reservoir.selected_phat = linear_to_luma(contribution);

        var combined_reservoir = GiReservoir::new();
        GiReservoir::update(&combined_reservoir, reservoir.selected_phat * reservoir.contribution_weight * reservoir.sample_count, &rng, reservoir.w_in_worldspace, reservoir.selected_phat);
        GiReservoir::update(&combined_reservoir, prev_reservoir.selected_phat * prev_reservoir.contribution_weight * prev_reservoir.sample_count, &rng, prev_reservoir.w_in_worldspace, prev_reservoir.selected_phat);
        combined_reservoir.sample_count = reservoir.sample_count + prev_reservoir.sample_count;
        if (combined_reservoir.selected_phat > 0.0) {
            combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
        }

        reservoirs_out[id] = PackedGiReservoir::new(combined_reservoir);
        if (constants.spatial_pass_count == 0 || true) {
            prev_reservoirs_out[id] = PackedGiReservoir::new(combined_reservoir);
        }
    } else {
        reservoirs_out[id] = PackedGiReservoir::new(reservoir);
        if (constants.spatial_pass_count == 0 || true) {
            prev_reservoirs_out[id] = PackedGiReservoir::new(reservoir);
        }
    }

    payload.rng = rng;
    payloads[id] = payload;
}