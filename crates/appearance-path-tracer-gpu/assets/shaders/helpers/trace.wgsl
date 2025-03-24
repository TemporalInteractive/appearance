///
/// BINDING DEPENDENCIES:
/// appearance-path-tracer-gpu::shared/vertex_pool_bindings
/// appearance-path-tracer-gpu::shared/material/material_pool_bindings
///

const MAX_NON_OPAQUE_DEPTH: u32 = 1;
const MAX_NON_OPAQUE_SHADOW_DEPTH: u32 = 1;

const TRACE_EPSILON: f32 = 1e-4;

fn safe_origin(origin: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    return origin + normal * TRACE_EPSILON;
}

fn safe_distance(distance: f32) -> f32 {
    return distance - TRACE_EPSILON * 2.0;
}

fn trace_shadow_ray_opaque(origin: vec3<f32>, direction: vec3<f32>, distance: f32, normal: vec3<f32>, scene: acceleration_structure) -> bool {
    var shadow_rq: ray_query;
    rayQueryInitialize(&shadow_rq, scene, RayDesc(0x4, 0xFFu, 0.0, safe_distance(distance), safe_origin(origin, normal), direction));
    rayQueryProceed(&shadow_rq);
    let intersection = rayQueryGetCommittedIntersection(&shadow_rq);
    return intersection.kind != RAY_QUERY_INTERSECTION_TRIANGLE;
}

fn trace_shadow_ray(_origin: vec3<f32>, direction: vec3<f32>, distance: f32, normal: vec3<f32>, scene: acceleration_structure) -> bool {
    var origin: vec3<f32> = _origin;

    if (MAX_NON_OPAQUE_SHADOW_DEPTH == 1) {
        return trace_shadow_ray_opaque(origin, direction, distance, normal, scene);
    }

    var travelled_distance: f32 = 0.0;
    var safe_origin_normal: vec3<f32> = normal;
    for (var step: u32 = 0; step < MAX_NON_OPAQUE_SHADOW_DEPTH; step += 1) {
        if (dot(safe_origin_normal, direction) < 0.0) {
            safe_origin_normal *= -1.0;
        }

        var rq: ray_query;
        rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, safe_distance(distance - travelled_distance), safe_origin(origin, safe_origin_normal), direction));
        rayQueryProceed(&rq);

        let intersection = rayQueryGetCommittedIntersection(&rq);
        if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[intersection.instance_custom_data];

            let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

            let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
            let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
            let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

            let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
            let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
            let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

            let tex_coord: vec2<f32> = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;

            let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + intersection.primitive_index];
            let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
            let luminance: f32 = MaterialDescriptor::color(material_descriptor, tex_coord).a;

            if (luminance < material_descriptor.alpha_cutoff) {
                var p01: vec3<f32> = v1.position - v0.position;
                var p02: vec3<f32> = v2.position - v0.position;
                safe_origin_normal = normalize(cross(p01, p02));

                // TODO: non-opaque geometry would be a better choice, not properly supported by wgpu yet
                origin += direction * intersection.t;
                travelled_distance += intersection.t;
                continue; // check if last? return false if so
            }

            return false;
        }

        return true;
    }

    return false;
}