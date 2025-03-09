@include ::random
@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir_di/di_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings

@include appearance-path-tracer-gpu::shared/nee

struct Constants {
    resolution: vec2<u32>,
    spatial_pass_count: u32,
    spatial_pass_idx: u32,
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
var<storage, read> in_reservoirs: array<PackedDiReservoir>;

@group(0)
@binding(5)
var<storage, read_write> out_reservoirs: array<PackedDiReservoir>;

@group(0)
@binding(6)
var<storage, read_write> prev_reservoirs: array<PackedDiReservoir>;

@group(0)
@binding(7)
var<storage, read> light_sample_ctxs: array<LightSampleCtx>;

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

    let reservoir: DiReservoir = PackedDiReservoir::unpack(in_reservoirs[flat_id]);

    var combined_reservoir = DiReservoir::new();
    var combined_sample_count: f32 = reservoir.sample_count;
    DiReservoir::update(&combined_reservoir, reservoir.selected_phat * reservoir.contribution_weight * reservoir.sample_count, &rng, reservoir.sample, reservoir.selected_phat);

    for (var i: u32 = 0; i < 4; i += 1) {
        let center_id = vec2<i32>(i32(id.x), i32(id.y));
        let offset = vec2<i32>(
            i32((random_uniform_float(&rng) * 2.0 - 1.0) * 30.0),
            i32((random_uniform_float(&rng) * 2.0 - 1.0) * 30.0)
        );
        let neighbour_id = vec2<u32>(clamp(center_id + offset, vec2<i32>(0), vec2<i32>(constants.resolution)));
        let flat_neighbour_id: u32 = neighbour_id.y * constants.resolution.x + neighbour_id.x;

        // TODO: gbuffer based rejection
        var neighbour_reservoir: DiReservoir = PackedDiReservoir::unpack(prev_reservoirs[flat_neighbour_id]);

        let w_out_worldspace: vec3<f32> = -direction;
        let w_in_worldspace: vec3<f32> = normalize(neighbour_reservoir.sample.point - hit_point_ws);
        let n_dot_l: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);
        if (n_dot_l > 0.0) {
            let sample_intensity = LightSample::intensity(neighbour_reservoir.sample, hit_point_ws);

            var shading_pdf: f32;
            let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
                tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, w_in_worldspace, &shading_pdf);
            let contribution: vec3<f32> = n_dot_l * reflectance;

            neighbour_reservoir.selected_phat = linear_to_luma(contribution * sample_intensity);
        } else {
            neighbour_reservoir.selected_phat = 0.0;
        }

        DiReservoir::update(&combined_reservoir, neighbour_reservoir.selected_phat * neighbour_reservoir.contribution_weight * neighbour_reservoir.sample_count, &rng, neighbour_reservoir.sample, neighbour_reservoir.selected_phat);
        combined_sample_count += neighbour_reservoir.sample_count;
    }

    combined_reservoir.sample_count = combined_sample_count;
    if (combined_reservoir.selected_phat > 0.0) {
        combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
    }

    out_reservoirs[flat_id] = PackedDiReservoir::new(combined_reservoir);
    if (constants.spatial_pass_idx == constants.spatial_pass_count - 1) {
        prev_reservoirs[flat_id] = PackedDiReservoir::new(combined_reservoir);
    }

    payload.rng = rng;
    payloads[flat_id] = payload;
}