@include ::random
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir_di/di_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings

@include appearance-path-tracer-gpu::shared/nee
@include appearance-path-tracer-gpu::shared/trace_helpers

struct Constants {
    ray_count: u32,
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
var<storage, read> light_sample_reservoirs: array<PackedDiReservoir>;

@group(0)
@binding(5)
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

    let di_reservoir: DiReservoir = PackedDiReservoir::unpack(light_sample_reservoirs[id]);
    let light_sample: LightSample = di_reservoir.sample;
    if (di_reservoir.contribution_weight == 0.0) { return; }

    let light_sample_ctx: LightSampleCtx = light_sample_ctxs[id];

    var accumulated: vec3<f32> = PackedRgb9e5::unpack(payload.accumulated);
    // Current payload throughput already has the next gi bounce reflection incorporated, take "previous" throughput from the light sample ctx
    let throughput: vec3<f32> = PackedRgb9e5::unpack(light_sample_ctx.throughput);
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

    let shadow_direction: vec3<f32> = normalize(light_sample.point - hit_point_ws);
    let shadow_distance: f32 = distance(light_sample.point, hit_point_ws);
    let n_dot_l: f32 = dot(shadow_direction, front_facing_shading_normal_ws);
    if (n_dot_l > 0.0) {
        if (trace_shadow_ray(hit_point_ws, shadow_direction, shadow_distance, scene)) {
            let w_out_worldspace: vec3<f32> = -direction;
            let w_in_worldspace: vec3<f32> = shadow_direction;

            var shading_pdf: f32;
            let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, w_in_worldspace, &shading_pdf);

            let light_intensity: vec3<f32> = LightSample::intensity(light_sample, hit_point_ws) * light_sample.emission;

            let contribution: vec3<f32> = throughput * reflectance * light_intensity * n_dot_l * di_reservoir.contribution_weight;
            accumulated += contribution;
        };
    }

    payload.accumulated = PackedRgb9e5::new(accumulated);
    payload.rng = rng;
    payloads[id] = payload;
}