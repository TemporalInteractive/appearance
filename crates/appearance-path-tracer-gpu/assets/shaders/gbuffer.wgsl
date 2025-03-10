@include appearance-packing::shared/packing

struct VertexOutput {
    @location(0) normal: vec3<f32>,
    @builtin(position) position: vec4<f32>,
};

struct PushConstant {
    model: mat4x4<f32>,
}

struct Constants {
    view_proj: mat4x4<f32>,
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

    let ws_position: vec4<f32> = pc.model * vec4<f32>(position.xyz, 1.0);

    var result: VertexOutput;
    result.position = constants.view_proj * ws_position;
    result.normal = PackedNormalizedXyz10::unpack(packed_normal, 0);
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(vertex.position.w, vertex.normal);
}