@include appearance-packing::shared/packing

struct VertexOutput {
    @location(0) normal: vec3<f32>,
    @location(1) position_ws: vec4<f32>,
    @location(2) prev_position_ws: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

struct PushConstant {
    model: mat4x4<f32>,
    prev_model: mat4x4<f32>,
}

struct Constants {
    view_proj: mat4x4<f32>,
    view: mat4x4<f32>,
    view_position: vec3<f32>,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

var<push_constant> pc : PushConstant;

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) _packed_normal: u32,
    @location(2) tex_coord: vec2<f32>,
    @location(3) _packed_tangent: u32,
    @location(4) tangent_handiness: f32,
    @builtin(instance_index) i: u32,
) -> VertexOutput {
    let packed_normal = PackedNormalizedXyz10(_packed_normal);
    let packed_tangent = PackedNormalizedXyz10(_packed_tangent);

    var result: VertexOutput;
    result.position_ws = pc.model * vec4<f32>(position.xyz, 1.0);
    result.prev_position_ws = pc.prev_model * vec4<f32>(position.xyz, 1.0);
    result.position = constants.view_proj * result.position_ws;
    result.normal = PackedNormalizedXyz10::unpack(packed_normal, 0);
    return result;
}

struct FragmentOutput {
    @location(0) depth_normal: vec4<f32>,
    @location(1) velocity_derivative: vec4<f32>,
};

@fragment
fn fs_main(vertex: VertexOutput) -> FragmentOutput {
    let depth_ws: f32 = distance(constants.view_position, vertex.position_ws.xyz);
    let normal_ws: vec3<f32> = vertex.normal;

    let prev_position_vs: vec3<f32> = (constants.view * vertex.prev_position_ws).xyz;
    let position_vs: vec3<f32> = (constants.view * vertex.position_ws).xyz;
    let velocity: vec3<f32> = prev_position_vs - position_vs;

    var result: FragmentOutput;
    result.depth_normal = vec4<f32>(depth_ws, vertex.normal);
    result.velocity_derivative = vec4<f32>(velocity, 0.0);
    return result;
}