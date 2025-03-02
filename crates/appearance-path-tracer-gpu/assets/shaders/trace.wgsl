@include ::random
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/diffuse_brdf
@include appearance-path-tracer-gpu::shared/disney_bsdf

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material_pool_bindings

struct Constants {
    ray_count: u32,
    bounce: u32,
    seed: u32,
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
var<storage, read_write> out_rays: array<Ray>;

@group(0)
@binding(3)
var<storage, read_write> payloads: array<Payload>;

@group(0)
@binding(4)
var scene: acceleration_structure;

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
    if (constants.bounce == 0) {
        let rng = pcg_hash(id ^ xor_shift_u32(constants.seed));
        payload = Payload::new(vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0), rng, 1);
    }

    if (payload.alive == 0) { return; } // TODO: indirect dispatch with pids

    var accumulated: vec3<f32> = PackedRgb9e5::unpack(payload.accumulated);
    var throughput: vec3<f32> = PackedRgb9e5::unpack(payload.throughput);
    var rng: u32 = payload.rng;

    for (var step: u32 = 0; step < 64; step += 1) {
        var rq: ray_query;
        rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, 1000.0, origin, direction));
        rayQueryProceed(&rq);

        let intersection = rayQueryGetCommittedIntersection(&rq);
        if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
            let vertex_pool_slice_index: u32 = intersection.instance_custom_data;
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[vertex_pool_slice_index];

            let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

            let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
            let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
            let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

            let tex_coord0: vec2<f32> = vertex_tex_coords[vertex_pool_slice.first_vertex + i0];
            let tex_coord1: vec2<f32> = vertex_tex_coords[vertex_pool_slice.first_vertex + i1];
            let tex_coord2: vec2<f32> = vertex_tex_coords[vertex_pool_slice.first_vertex + i2];
            let tex_coord: vec2<f32> = tex_coord0 * barycentrics.x + tex_coord1 * barycentrics.y + tex_coord2 * barycentrics.z;

            let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + intersection.primitive_index];
            let material: Material = Material::from_material_descriptor(material_descriptors[material_idx], tex_coord);

            if (material.base_color.a < material.alpha_cutoff) {
                origin += direction * (intersection.t + 0.001);
                continue;
            }

            let normal0: vec3<f32> = vertex_normals[vertex_pool_slice.first_vertex + i0].xyz;
            let normal1: vec3<f32> = vertex_normals[vertex_pool_slice.first_vertex + i1].xyz;
            let normal2: vec3<f32> = vertex_normals[vertex_pool_slice.first_vertex + i2].xyz;
            var normal: vec3<f32> = normalize(normal0 * barycentrics.x + normal1 * barycentrics.y + normal2 * barycentrics.z);

            let trans_transform = mat4x4<f32>(
                vec4<f32>(intersection.world_to_object[0], 0.0),
                vec4<f32>(intersection.world_to_object[1], 0.0),
                vec4<f32>(intersection.world_to_object[2], 0.0),
                vec4<f32>(0.0, 0.0, 0.0, 1.0)
            );
            let inv_trans_transform = transpose(trans_transform);
            normal = normalize((inv_trans_transform * vec4<f32>(normal, 1.0)).xyz);

            let tangent_to_world: mat3x3<f32> = build_orthonormal_basis(normal);
            let world_to_tangent: mat3x3<f32> = transpose(tangent_to_world);

            let diffuse_lobe = DiffuseLobe::new(material.base_color.rgb);
            let bsdf_sample: BsdfSample = DiffuseLobe::sample(diffuse_lobe, random_uniform_float2(&rng));
            var bsdf_eval: BsdfEval = DiffuseLobe::eval(diffuse_lobe);

            var w_in_worldspace: vec3<f32>;
            let sample_valid: bool = apply_bsdf(bsdf_sample, bsdf_eval, tangent_to_world, normal, &throughput, &w_in_worldspace);

            if (sample_valid) {
                let point = origin + direction * intersection.t;
                let out_ray = Ray::new(point + w_in_worldspace * 0.0001, w_in_worldspace);
                out_rays[id] = out_ray;
            } else {
                payload.alive = 0;
            }
        } else {
            let a: f32 = 0.5 * (direction.y + 1.0);
            let color = (1.0 - a) * vec3<f32>(1.0, 1.0, 1.0) + a * vec3<f32>(0.5, 0.7, 1.0);
            accumulated += throughput * color * 4.0;
            payload.alive = 0;
        }

        break;
    }
    
    payload.accumulated = PackedRgb9e5::new(accumulated);
    payload.throughput = PackedRgb9e5::new(throughput);
    payload.rng = rng;
    payloads[id] = payload;
}