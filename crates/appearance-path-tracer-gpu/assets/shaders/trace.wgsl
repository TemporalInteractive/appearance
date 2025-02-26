@include appearance-path-tracer-gpu::ray

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

    var rq: ray_query;
    rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.01, 1000.0, ray.origin, ray.direction));
    rayQueryProceed(&rq);

    let intersection = rayQueryGetCommittedIntersection(&rq);
    if (intersection.kind != RAY_QUERY_INTERSECTION_NONE) {
        payloads[id] = Payload::new(vec3<f32>(0.0, 1.0, 0.5));
    } else {
        let a: f32 = 0.5 * (ray.direction.y + 1.0);
        let color = (1.0 - a) * vec3<f32>(1.0, 1.0, 1.0) + a * vec3<f32>(0.5, 0.7, 1.0);
        payloads[id] = Payload::new(color);
    }
}