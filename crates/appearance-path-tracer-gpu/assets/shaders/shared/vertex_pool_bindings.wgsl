@include appearance-path-tracer-gpu::shared/vertex_pool

@group(1)
@binding(0)
var<storage, read> vertex_positions: array<vec4<f32>>;

@group(1)
@binding(1)
var<storage, read> vertex_normals: array<vec4<f32>>;

@group(1)
@binding(2)
var<storage, read> vertex_tex_coords: array<vec2<f32>>;

@group(1)
@binding(3)
var<storage, read> vertex_indices: array<u32>;

@group(1)
@binding(4)
var<storage, read> triangle_material_indices: array<u32>;

@group(1)
@binding(5)
var<storage, read> vertex_pool_slices: array<VertexPoolSlice>;