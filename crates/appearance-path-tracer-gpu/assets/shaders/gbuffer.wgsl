@include appearance-packing::shared/packing

struct VertexOutput {
    @location(0) position_cs: vec4<f32>,
    @location(1) prev_position_cs: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

struct PushConstant {
    model: mat4x4<f32>,
    prev_model: mat4x4<f32>,
}

struct Constants {
    view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
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
    result.position_cs = constants.view_proj * pc.model * vec4<f32>(position.xyz, 1.0);
    result.prev_position_cs = constants.prev_view_proj * pc.prev_model * vec4<f32>(position.xyz, 1.0);
    result.position = result.position_cs;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    var prev_position_ss: vec4<f32> = (vertex.prev_position_cs / vertex.prev_position_cs.w + 1.0) / 2.0;
    prev_position_ss = vec4<f32>(prev_position_ss.x, 1.0 - prev_position_ss.y, prev_position_ss.zw);
    var position_ss: vec4<f32> = (vertex.position_cs / vertex.position_cs.w + 1.0) / 2.0;
    position_ss = vec4<f32>(position_ss.x, 1.0 - position_ss.y, position_ss.zw);
    let velocity: vec2<f32> = (position_ss - prev_position_ss).xy;

    return vec4<f32>(velocity, 0.0, 0.0);
}