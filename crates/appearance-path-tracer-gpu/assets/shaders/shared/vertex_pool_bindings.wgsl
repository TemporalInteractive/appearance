@include appearance-path-tracer-gpu::shared/vertex_pool
@include appearance-path-tracer-gpu::shared/light_sample

struct VertexPoolConstants {
    num_emissive_triangle_instances: u32,
    num_emissive_triangles: u32,
    _padding1: u32,
    _padding2: u32,
}

struct EmissiveTriangleInstance {
    transform: mat4x4<f32>, // TODO: put in separate buffer, also make 4x3
    vertex_pool_slice_idx: u32,
    num_triangles: u32,
    cdf: f32,
    _padding0: u32,
}

@group(1)
@binding(0)
var<uniform> vertex_pool_constants: VertexPoolConstants;

@group(1)
@binding(1)
var<storage, read> vertices: array<PackedVertex>;

@group(1)
@binding(2)
var<storage, read> vertex_indices: array<u32>;

@group(1)
@binding(3)
var<storage, read> triangle_material_indices: array<u32>;

@group(1)
@binding(4)
var<storage, read> emissive_triangle_instances: array<EmissiveTriangleInstance>;

@group(1)
@binding(5)
var<storage, read> vertex_pool_slices: array<VertexPoolSlice>;

fn _calculate_bitangent(normal: vec3<f32>, tangent: vec4<f32>) -> vec3<f32> {
    var bitangent: vec3<f32> = cross(normal, tangent.xyz);
    return bitangent * tangent.w;
}

fn VertexPoolBindings::load_tbn(v0: Vertex, v1: Vertex, v2: Vertex, barycentrics: vec3<f32>) -> mat3x3<f32> {
    let normal: vec3<f32> = v0.normal * barycentrics.x + v1.normal * barycentrics.y + v2.normal * barycentrics.z;
    let tangent: vec3<f32> = v0.tangent.xyz * barycentrics.x + v1.tangent.xyz * barycentrics.y + v2.tangent.xyz * barycentrics.z;

    let bitangent0: vec3<f32> = _calculate_bitangent(v0.normal, v0.tangent);
    let bitangent1: vec3<f32> = _calculate_bitangent(v1.normal, v1.tangent);
    let bitangent2: vec3<f32> = _calculate_bitangent(v2.normal, v2.tangent);
    let bitangent: vec3<f32> = bitangent0 * barycentrics.x + bitangent1 * barycentrics.y + bitangent2 * barycentrics.z;

    return mat3x3<f32>(tangent, bitangent, normal);
}