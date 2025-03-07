@include ::random
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/diffuse_brdf
@include appearance-path-tracer-gpu::shared/disney_bsdf

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings
@include appearance-path-tracer-gpu::shared/material_pool_bindings
@include appearance-path-tracer-gpu::shared/sky_bindings

@include appearance-path-tracer-gpu::shared/nee

struct Constants {
    ray_count: u32,
    bounce: u32,
    seed: u32,
    sample: u32,
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
        if (constants.sample == 0) {
            payload.accumulated = PackedRgb9e5::new(vec3<f32>(0.0));
        }
        payload.throughput = PackedRgb9e5::new(vec3<f32>(1.0));
        payload.rng = pcg_hash(id ^ xor_shift_u32(constants.seed));
        payload.alive = 1;
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
            let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
            let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord);

            if (material.luminance < material.alpha_cutoff) {
                // TODO: non-opaque geometry would be a better choice, not properly supported by wgpu yet
                origin += direction * (intersection.t + 0.001);
                continue;
            }

            if (constants.bounce == 0) {
                accumulated += throughput * material.emission;
            }

            // Load tangent, bitangent and normal in local space
            let tbn: mat3x3<f32> = VertexPoolBindings::load_tbn(
                vertex_pool_slice.first_vertex + i0,
                vertex_pool_slice.first_vertex + i1,
                vertex_pool_slice.first_vertex + i2,
                barycentrics
            );

            // Calculate local to world matrix, inversed and transposed
            let local_to_world_inv = mat4x4<f32>(
                vec4<f32>(intersection.world_to_object[0], 0.0),
                vec4<f32>(intersection.world_to_object[1], 0.0),
                vec4<f32>(intersection.world_to_object[2], 0.0),
                vec4<f32>(0.0, 0.0, 0.0, 1.0)
            );
            let local_to_world_inv_trans: mat4x4<f32> = transpose(local_to_world_inv);

            // World space tangent, bitangent and normal. Note that these are not front facing yet
            let hit_tangent_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[0], 1.0)).xyz);
            let hit_bitangent_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[1], 1.0)).xyz);
            var hit_normal_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[2], 1.0)).xyz);
            let hit_point_ws = origin + direction * intersection.t;

            let hit_tangent_to_world = mat3x3<f32>(
                hit_tangent_ws,
                hit_bitangent_ws,
                hit_normal_ws
            );

            // Apply normal mapping when available, unlike the name suggest, still not front facing
            var front_facing_normal_ws: vec3<f32> = hit_normal_ws;
            var front_facing_shading_normal_ws: vec3<f32> = MaterialDescriptor::apply_normal_mapping(material_descriptor, tex_coord, hit_normal_ws, hit_tangent_to_world);

            let w_out_worldspace: vec3<f32> = -direction;

            // Make sure the hit normal and normal mapped normal are front facing
            let back_face: bool = dot(w_out_worldspace, hit_normal_ws) < 0.0;
            if (back_face) {
                front_facing_normal_ws *= -1.0;
                front_facing_shading_normal_ws *= -1.0;
            }

            // Construct tangent <-> world matrices
            let tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_shading_normal_ws);
            let world_to_tangent: mat3x3<f32> = transpose(tangent_to_world);

            var clearcoat_tangent_to_world: mat3x3<f32>;
            var clearcoat_world_to_tangent: mat3x3<f32>;
            if (material_descriptor.clearcoat > 0.0) {
                if (material_descriptor.clearcoat_normal_texture == INVALID_TEXTURE) {
                    clearcoat_tangent_to_world = build_orthonormal_basis(front_facing_normal_ws);
                    clearcoat_world_to_tangent = transpose(clearcoat_tangent_to_world);
                } else if (material_descriptor.clearcoat_normal_texture == material_descriptor.normal_texture) {
                    clearcoat_tangent_to_world = tangent_to_world;
                    clearcoat_world_to_tangent = world_to_tangent;
                } else {
                    var front_facing_clearcoat_normal_ws: vec3<f32> = MaterialDescriptor::apply_clearcoat_normal_mapping(material_descriptor, tex_coord, hit_normal_ws, hit_tangent_to_world);
                    if (back_face) {
                        front_facing_clearcoat_normal_ws *= -1.0;
                    }
                    clearcoat_tangent_to_world = build_orthonormal_basis(front_facing_clearcoat_normal_ws);
                    clearcoat_world_to_tangent = transpose(clearcoat_tangent_to_world);
                }
            }

            let disney_bsdf = DisneyBsdf::from_material(material);

            let light_sample: LightSample = Nee::sample(random_uniform_float(&rng), random_uniform_float(&rng), random_uniform_float(&rng),
                vec2<f32>(random_uniform_float(&rng), random_uniform_float(&rng)), hit_point_ws);
            let shadow_direction: vec3<f32> = light_sample.direction;
            let shadow_origin: vec3<f32> = hit_point_ws + shadow_direction * 0.0001;
            let n_dot_l: f32 = dot(shadow_direction, front_facing_shading_normal_ws);
            if (n_dot_l > 0.0 && light_sample.pdf > 0.0) {
                var shadow_rq: ray_query;
                rayQueryInitialize(&shadow_rq, scene, RayDesc(0x4, 0xFFu, 0.0, light_sample.distance, shadow_origin, shadow_direction));
                rayQueryProceed(&shadow_rq);
                let intersection = rayQueryGetCommittedIntersection(&shadow_rq);
                if (intersection.kind != RAY_QUERY_INTERSECTION_TRIANGLE) {
                    let w_in_worldspace: vec3<f32> = shadow_direction;

                    var shading_pdf: f32;
                    let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                        w_out_worldspace, w_in_worldspace, &shading_pdf);

                    let light_intensity: vec3<f32> = Nee::intensity(light_sample) * light_sample.emission;

                    let contribution: vec3<f32> = throughput * reflectance * light_intensity * n_dot_l / light_sample.pdf;
                    accumulated += contribution;
                };
            }

            var w_in_worldspace: vec3<f32>;
            var pdf: f32;
            var specular: bool;
            let reflectance: vec3<f32> = DisneyBsdf::sample(disney_bsdf,
                front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, intersection.t, back_face,
                random_uniform_float(&rng), random_uniform_float(&rng), random_uniform_float(&rng),
                &w_in_worldspace, &pdf, &specular
            );

            let sample_valid: bool = pdf > 1e-6;
            if (sample_valid) {
                let cos_in: f32 = abs(dot(front_facing_shading_normal_ws, w_in_worldspace));
                let contribution: vec3<f32> = (1.0 / pdf) * reflectance * cos_in;
                throughput *= contribution;
            
                let out_ray = Ray::new(hit_point_ws + w_in_worldspace * 0.0001, w_in_worldspace);
                out_rays[id] = out_ray;
            } else {
                payload.alive = 0;
            }
        } else {
            let color = Sky::sky(direction, true);
            accumulated += throughput * color;
            payload.alive = 0;
        }

        break;
    }
    
    payload.accumulated = PackedRgb9e5::new(accumulated);
    payload.throughput = PackedRgb9e5::new(throughput);
    payload.rng = rng;
    payloads[id] = payload;
}