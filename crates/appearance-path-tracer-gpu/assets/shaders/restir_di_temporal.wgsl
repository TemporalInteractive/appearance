@include ::random
@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/gbuffer
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir/di_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings
@include appearance-path-tracer-gpu::shared/gbuffer_bindings

@include appearance-path-tracer-gpu::helpers/nee
@include appearance-path-tracer-gpu::helpers/trace

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
var<storage, read> reservoirs_in: array<PackedDiReservoir>;

@group(0)
@binding(5)
var<storage, read_write> reservoirs_out: array<PackedDiReservoir>;

@group(0)
@binding(6)
var<storage, read> prev_reservoirs_in: array<PackedDiReservoir>;

@group(0)
@binding(7)
var<storage, read_write> prev_reservoirs_out: array<PackedDiReservoir>;

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

    let light_sample_ctx: LightSampleCtx = light_sample_ctxs[flat_id];
    
    var rng: u32 = payload.rng;

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

    let reservoir: DiReservoir = PackedDiReservoir::unpack(reservoirs_in[flat_id]);

    var valid_prev_reservoir: bool = false;
    var prev_id: u32;

    let velocity: vec2<f32> = textureLoad(velocity_texture, vec2<i32>(id)).xy;
    let prev_id_unclamped = vec2<i32>(vec2<f32>(id) - (vec2<f32>(constants.resolution) * velocity) + (vec2<f32>(random_uniform_float(&rng), random_uniform_float(&rng)) * 0.5));
    if (all(prev_id_unclamped >= vec2<i32>(0)) && all(prev_id_unclamped < vec2<i32>(constants.resolution))) {
        prev_id = u32(prev_id_unclamped.y) * constants.resolution.x + u32(prev_id_unclamped.x);

        let current_gbuffer_texel: GBufferTexel = gbuffer[flat_id];
        let prev_gbuffer_texel: GBufferTexel = prev_gbuffer[prev_id];
        let prev_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(prev_gbuffer_texel.normal_ws, 0);
        if (constants.unbiased == 0) {
            let current_depth_cs: f32 = GBufferTexel::depth_cs(current_gbuffer_texel, 0.001, 10000.0);
            let prev_depth_cs: f32 = GBufferTexel::depth_cs(prev_gbuffer_texel, 0.001, 10000.0);
            let valid_delta_depth: bool = (abs(current_depth_cs - prev_depth_cs) / current_depth_cs) < 0.1;
            let current_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(current_gbuffer_texel.normal_ws, 0);
            let valid_delta_normal: bool = dot(current_normal_ws, prev_normal_ws) > 0.906; // 25 degrees

            valid_prev_reservoir = valid_delta_depth && valid_delta_normal;
        } else {
            valid_prev_reservoir = true;
        }
    }

    if (valid_prev_reservoir) {
        var prev_reservoir: DiReservoir = PackedDiReservoir::unpack(prev_reservoirs_in[prev_id]);
        prev_reservoir.sample_count = min(prev_reservoir.sample_count, 30.0);

        let w_out_worldspace: vec3<f32> = -direction;
        let w_in_worldspace: vec3<f32> = normalize(prev_reservoir.sample.point - hit_point_ws);

        var visibility: bool = true;
        if (constants.unbiased > 0) {
            let distance: f32 = distance(prev_reservoir.sample.point, hit_point_ws);

            if (!trace_shadow_ray(hit_point_ws, w_in_worldspace, distance, scene)) {
                visibility = false;
            }
        }

        let n_dot_l: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);
        if (n_dot_l > 0.0 && visibility) {
            let sample_intensity = LightSample::intensity(prev_reservoir.sample, hit_point_ws);

            var shading_pdf: f32;
            let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
                tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, w_in_worldspace, &shading_pdf);
            let contribution: vec3<f32> = n_dot_l * reflectance;

            prev_reservoir.selected_phat = linear_to_luma(contribution * sample_intensity);
        } else {
            prev_reservoir.selected_phat = 0.0;
        }

        var combined_reservoir = DiReservoir::new();
        DiReservoir::update(&combined_reservoir, reservoir.selected_phat * reservoir.contribution_weight * reservoir.sample_count, &rng, reservoir.sample, reservoir.selected_phat);
        DiReservoir::update(&combined_reservoir, prev_reservoir.selected_phat * prev_reservoir.contribution_weight * prev_reservoir.sample_count, &rng, prev_reservoir.sample, prev_reservoir.selected_phat);
        combined_reservoir.sample_count = reservoir.sample_count + prev_reservoir.sample_count;
        if (combined_reservoir.selected_phat > 0.0 && combined_reservoir.sample_count * combined_reservoir.weight_sum > 0.0) {
            combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
        }

        reservoirs_out[flat_id] = PackedDiReservoir::new(combined_reservoir);
        if (constants.spatial_pass_count == 0) {
            prev_reservoirs_out[flat_id] = PackedDiReservoir::new(combined_reservoir);
        }
    } else {
        reservoirs_out[flat_id] = PackedDiReservoir::new(reservoir);

        if (constants.spatial_pass_count == 0) {
            prev_reservoirs_out[flat_id] = PackedDiReservoir::new(reservoir);
        }
    }

    payload.rng = rng;
    payloads[flat_id] = payload;
}