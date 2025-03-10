@include appearance-packing::shared/packing

struct VertexOutput {
    @location(0) normal: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) blas_instance_idx: u32,
    @location(3) ws_position: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

struct PushConstant {
    model: mat4x4<f32>,
    blas_instance_idx: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

struct Constants {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

var<push_constant> pc : PushConstant;

@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) tangent: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) tex_coord: vec4<f32>,
    @location(5) joints: vec4<u32>,
    @location(6) weights: vec4<f32>,
    @builtin(instance_index) i: u32,
) -> VertexOutput {
    var result: VertexOutput;
    result.ws_position = pc.model * vec4<f32>(position.xyz, 1.0);
    result.position = constants.view_proj * result.ws_position;
    result.normal = normal.xyz;
    result.tex_coord = tex_coord.xy;
    result.blas_instance_idx = pc.blas_instance_idx;
    return result;
}

fn sq_distance(a: vec3<f32>, b: vec3<f32>) -> f32 {
    var delta: vec3<f32> = b - a;
    return dot(delta, delta);
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    var sq_depth: f32 = 1.0 / sq_distance(constants.camera_pos, vertex.ws_position.xyz);

    var packed_normal: f32 = bitcast<f32>(pack_normalized_xyz10(vertex.normal, 0u).data);
    var packed_tex_coord: f32 = bitcast<f32>(pack_uv16(vertex.tex_coord.xy).data);
    return vec4<f32>(sq_depth, packed_normal, packed_tex_coord, bitcast<f32>(vertex.blas_instance_idx));
}