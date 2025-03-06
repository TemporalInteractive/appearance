@include appearance-path-tracer-gpu::shared/vertex_pool

struct VertexPoolConstants {
    num_emissive_triangle_instances: u32,
    num_emissive_triangles: u32,
    _padding1: u32,
    _padding2: u32,
}

struct EmissiveTriangleInstance {
    transform: mat4x4<f32>,
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
var<storage, read> vertex_positions: array<vec4<f32>>;

@group(1)
@binding(2)
var<storage, read> vertex_normals: array<vec4<f32>>;

@group(1)
@binding(3)
var<storage, read> vertex_tangents: array<vec4<f32>>;

@group(1)
@binding(4)
var<storage, read> vertex_tex_coords: array<vec2<f32>>;

@group(1)
@binding(5)
var<storage, read> vertex_indices: array<u32>;

@group(1)
@binding(6)
var<storage, read> triangle_material_indices: array<u32>;

@group(1)
@binding(7)
var<storage, read> emissive_triangle_instances: array<EmissiveTriangleInstance>;

@group(1)
@binding(8)
var<storage, read> vertex_pool_slices: array<VertexPoolSlice>;

fn _calculate_bitangent(normal: vec3<f32>, tangent: vec4<f32>) -> vec3<f32> {
    var bitangent: vec3<f32> = cross(normal, tangent.xyz);
    return bitangent * tangent.w;
}

fn VertexPoolBindings::load_tbn(v0: u32, v1: u32, v2: u32, barycentrics: vec3<f32>) -> mat3x3<f32> {
    let normal0: vec3<f32> = vertex_normals[v0].xyz;
    let normal1: vec3<f32> = vertex_normals[v1].xyz;
    let normal2: vec3<f32> = vertex_normals[v2].xyz;
    let normal: vec3<f32> = normal0 * barycentrics.x + normal1 * barycentrics.y + normal2 * barycentrics.z;

    let tangent0: vec4<f32> = vertex_tangents[v0];
    let tangent1: vec4<f32> = vertex_tangents[v1];
    let tangent2: vec4<f32> = vertex_tangents[v2];
    let tangent: vec3<f32> = tangent0.xyz * barycentrics.x + tangent1.xyz * barycentrics.y + tangent2.xyz * barycentrics.z;

    let bitangent0: vec3<f32> = _calculate_bitangent(normal0, tangent0);
    let bitangent1: vec3<f32> = _calculate_bitangent(normal1, tangent1);
    let bitangent2: vec3<f32> = _calculate_bitangent(normal2, tangent2);
    let bitangent: vec3<f32> = bitangent0 * barycentrics.x + bitangent1 * barycentrics.y + bitangent2 * barycentrics.z;

    return mat3x3<f32>(tangent, bitangent, normal);
}