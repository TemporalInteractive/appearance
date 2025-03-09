///
/// BINDING DEPENDENCIES:
/// appearance-path-tracer-gpu::shared/vertex_pool_bindings
/// appearance-path-tracer-gpu::shared/material/material_pool_bindings
///

const MAX_NON_OPAQUE_DEPTH: u32 = 1;

fn trace_shadow_ray_opaque(origin: vec3<f32>, direction: vec3<f32>, distance: f32, scene: acceleration_structure) -> bool {
    var shadow_rq: ray_query;
    rayQueryInitialize(&shadow_rq, scene, RayDesc(0x4, 0xFFu, 0.0, distance, origin + direction * 0.00001, direction));
    rayQueryProceed(&shadow_rq);
    let intersection = rayQueryGetCommittedIntersection(&shadow_rq);
    return intersection.kind != RAY_QUERY_INTERSECTION_TRIANGLE;
}

fn trace_shadow_ray(_origin: vec3<f32>, direction: vec3<f32>, distance: f32, scene: acceleration_structure) -> bool {
    var origin: vec3<f32> = _origin;

    if (MAX_NON_OPAQUE_DEPTH == 1) {
        return trace_shadow_ray_opaque(origin, direction, distance, scene);
    }

    var travelled_distance: f32 = 0.0;
    for (var step: u32 = 0; step < MAX_NON_OPAQUE_DEPTH; step += 1) {
        var rq: ray_query;
        rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, distance - travelled_distance, origin + direction * 0.00001, direction));
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
            let luminance: f32 = MaterialDescriptor::color(material_descriptor, tex_coord).w;

            if (luminance < material_descriptor.alpha_cutoff) {
                // TODO: non-opaque geometry would be a better choice, not properly supported by wgpu yet
                origin += direction * intersection.t;
                travelled_distance += intersection.t;
                continue;
            }

            return false;
        }

        return true;
    }

    return true;
}