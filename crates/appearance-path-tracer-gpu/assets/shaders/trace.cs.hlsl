#include "appearance-render-loop/block.hlsl"
#include "ray.hlsl"

[[vk::binding(0, 0)]]
cbuffer Constants {
    uint rayCount;
    uint _padding0;
    uint _padding1;
    uint _padding2;
};

[[vk::binding(1, 0)]]
RWStructuredBuffer<Ray> rays;

[[vk::binding(2, 0)]]
RWStructuredBuffer<Payload> payloads;

[numthreads(128, 1, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint id = global_id.x;
    if (id >= rayCount) return;
    
    Ray ray = rays[id];
    payloads[id].accumulated = lerp(float3(0.7, 0.7, 0.8), float3(1.0, 1.0, 1.0), ray.direction.y * 0.5 + 0.5);
}