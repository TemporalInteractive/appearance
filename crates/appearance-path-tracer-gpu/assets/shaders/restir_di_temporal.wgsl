@include ::random
@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/gbuffer
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir_di/di_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings

@include appearance-path-tracer-gpu::shared/nee

struct Constants {
    resolution: vec2<u32>,
    spatial_pass_count: u32,
    _padding0: u32,
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
var<storage, read_write> reservoirs: array<PackedDiReservoir>;

@group(0)
@binding(5)
var<storage, read_write> prev_reservoirs: array<PackedDiReservoir>;

@group(0)
@binding(6)
var<storage, read> light_sample_ctxs: array<LightSampleCtx>;

@group(0)
@binding(7)
var gbuffer: texture_storage_2d<rgba32float, read>;

@group(0)
@binding(8)
var prev_gbuffer: texture_storage_2d<rgba32float, read>;

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

    let reservoir: DiReservoir = PackedDiReservoir::unpack(reservoirs[flat_id]);

    let current_gbuffer_texel: GBufferTexel = GBufferTexel::new(textureLoad(gbuffer, vec2<i32>(id)));
    let prev_gbuffer_texel: GBufferTexel = GBufferTexel::new(textureLoad(prev_gbuffer, vec2<i32>(id))); // TODO: velocity mapping
    let current_depth_cs: f32 = current_gbuffer_texel.depth_ws;
    let prev_depth_cs: f32 = prev_gbuffer_texel.depth_ws;
    let valid_delta_depth: bool = (abs(current_depth_cs - prev_depth_cs) / current_depth_cs) < 0.1;
    let current_normal_ws: vec3<f32> = current_gbuffer_texel.normal_ws;
    let prev_normal_ws: vec3<f32> = prev_gbuffer_texel.normal_ws;
    let valid_delta_normal: bool = dot(current_normal_ws, prev_normal_ws) > 0.906; // 25 degrees

    let valid_prev_reservoir: bool = valid_delta_depth && valid_delta_normal;
    if (valid_prev_reservoir) {
        // TODO: velocity mapping
        var prev_reservoir: DiReservoir = PackedDiReservoir::unpack(prev_reservoirs[flat_id]);
        prev_reservoir.sample_count = min(prev_reservoir.sample_count, 20.0 * reservoir.sample_count);

        let w_out_worldspace: vec3<f32> = -direction;
        let w_in_worldspace: vec3<f32> = normalize(prev_reservoir.sample.point - hit_point_ws);
        let n_dot_l: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);
        if (n_dot_l > 0.0) {
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
        if (combined_reservoir.selected_phat > 0.0) {
            combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
        }

        reservoirs[flat_id] = PackedDiReservoir::new(combined_reservoir);
        if (constants.spatial_pass_count == 0) {
            prev_reservoirs[flat_id] = PackedDiReservoir::new(combined_reservoir);
        }

        payload.rng = rng;
        payloads[flat_id] = payload;
    } else if (constants.spatial_pass_count == 0) {
        prev_reservoirs[flat_id] = PackedDiReservoir::new(reservoir);
    }
}