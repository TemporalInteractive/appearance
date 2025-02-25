struct PsInput {
    float4 position : SV_Position;
    float2 uv: TEXCOORD;
};

[[vk::binding(0, 0)]]
Texture2D texture;

[[vk::binding(1, 0)]]
SamplerState linearSampler;

float4 main(PsInput IN) : SV_Target {
    float3 color = texture.Sample(linearSampler, IN.uv).rgb;
    return float4(color, 1.0);
}