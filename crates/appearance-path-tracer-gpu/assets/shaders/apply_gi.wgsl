@include ::random
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/material/disney_bsdf
@include appearance-path-tracer-gpu::shared/restir/gi_reservoir

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings

@include appearance-path-tracer-gpu::helpers/nee
@include appearance-path-tracer-gpu::helpers/trace

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
var<storage, read_write> in_rays: array<Ray>;

@group(0)
@binding(2)
var<storage, read_write> payloads: array<Payload>;

@group(0)
@binding(3)
var scene: acceleration_structure;

@group(0)
@binding(4)
var<storage, read> gi_reservoirs: array<PackedGiReservoir>;

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

    let gi_reservoir: GiReservoir = PackedGiReservoir::unpack(gi_reservoirs[id]);
    let w_in_worldspace: vec3<f32> = gi_reservoir.w_in_worldspace;
    if (gi_reservoir.contribution_weight > 0.0) {
        let light_sample_ctx: LightSampleCtx = light_sample_ctxs[id];

        var throughput: vec3<f32> = PackedRgb9e5::unpack(payload.throughput);

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

        let w_out_worldspace: vec3<f32> = -direction;

        var shading_pdf: f32;
        let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
            w_out_worldspace, w_in_worldspace, &shading_pdf);

        let cos_in: f32 = abs(dot(front_facing_shading_normal_ws, w_in_worldspace));
        let contribution: vec3<f32> = reflectance * cos_in * gi_reservoir.contribution_weight;
        throughput *= contribution;
        payload.throughput = PackedRgb9e5::new(throughput);

        let out_ray = Ray::new(hit_point_ws + w_in_worldspace * 0.0001, w_in_worldspace);
        in_rays[id] = out_ray;
    } else {
        payload.t = -1.0;
    }

    payloads[id] = payload;
}