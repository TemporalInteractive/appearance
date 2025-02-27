@include appearance-path-tracer-gpu::shared/ray

@include appearance-path-tracer-gpu::shared/vertex_pool_bindings

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
var<storage, read_write> rays: array<Ray>;

@group(0)
@binding(2)
var<storage, read_write> payloads: array<Payload>;

@group(0)
@binding(3)
var scene: acceleration_structure;

@compute
@workgroup_size(128)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: u32 = global_id.x;
    if (id >= constants.ray_count) { return; }

    let ray: Ray = rays[id];
    let origin: vec3<f32> = ray.origin;
    let direction: vec3<f32> = PackedNormalizedXyz10::unpack(ray.direction, 0);

    var rq: ray_query;
    rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.01, 1000.0, origin, direction));
    rayQueryProceed(&rq);

    let intersection = rayQueryGetCommittedIntersection(&rq);
    if (intersection.kind != RAY_QUERY_INTERSECTION_NONE) {
        let vertex_pool_slice_index: u32 = intersection.instance_custom_index;
        let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[vertex_pool_slice_index];

        let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
        let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
        let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

        let normal0: vec3<f32> = vertex_normals[vertex_pool_slice.first_vertex + i0].xyz;
        let normal1: vec3<f32> = vertex_normals[vertex_pool_slice.first_vertex + i1].xyz;
        let normal2: vec3<f32> = vertex_normals[vertex_pool_slice.first_vertex + i2].xyz;

        let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

        var normal: vec3<f32> = normalize(normal0 * barycentrics.x + normal1 * barycentrics.y + normal2 * barycentrics.z);

        var trans_transform = mat4x4<f32>(
            vec4<f32>(intersection.world_to_object[0], 0.0),
            vec4<f32>(intersection.world_to_object[1], 0.0),
            vec4<f32>(intersection.world_to_object[2], 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        );
        var inv_trans_transform = transpose(trans_transform);
        normal = normalize((inv_trans_transform * vec4<f32>(normal, 1.0)).xyz);

        let color = normal * 0.5 + 0.5;

        payloads[id] = Payload::new(color);
    } else {
        let a: f32 = 0.5 * (direction.y + 1.0);
        let color = (1.0 - a) * vec3<f32>(1.0, 1.0, 1.0) + a * vec3<f32>(0.5, 0.7, 1.0);
        payloads[id] = Payload::new(color);
    }
}