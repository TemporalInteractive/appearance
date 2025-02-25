#include "appearance-render-loop/block.hlsl"
#include "ray.hlsl"

[[vk::binding(0, 0)]]
cbuffer Constants {
    float4x4 invView;
    float4x4 invProj;
    uint2 resolution;
    uint _padding0;
    uint _padding1;
};

[[vk::binding(1, 0)]]
RWStructuredBuffer<Ray> rays;

[numthreads(16, 16, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint2 id = global_id.xy;
    if (any(id >= resolution)) return;

    float2 pixelCenter = float2(float(id.x) + 0.5, float(id.y) + 0.5);
    float2 uv = pixelCenter / float2(float(resolution.x), float(resolution.y));
    uv.y = -uv.y;
    
    float4 origin = mul(invView, float4(0.0, 0.0, 0.0, 1.0));
    float4 target = mul(invProj, float4(uv, 1.0, 1.0));
    float4 direction = mul(invView, float4(normalize(target.xyz), 0.0));

    rays[id.y * resolution.x + id.x] = Ray::_new(origin.xyz, direction.xyz);
}