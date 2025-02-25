struct VsOutput {
    float4 position : SV_Position;
    float2 uv: TEXCOORD;
};

// outputs a full screen triangle with screen-space coordinates
// input: three empty vertices
VsOutput main(uint vertexID : SV_VertexID) {
    VsOutput result;
    result.uv = float2((vertexID << 1) & 2, vertexID & 2);
    result.position = float4(result.uv * float2(2.0f, -2.0f) + float2(-1.0f, 1.0f), 0.0f, 1.0f);
    return result;
}