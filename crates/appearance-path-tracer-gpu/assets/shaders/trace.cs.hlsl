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

    float a = 0.5 * (ray.direction.y + 1.0);
    float3 color = (1.0 - a) * float3(1.0, 1.0, 1.0) + a * float3(0.5, 0.7, 1.0);

    payloads[id].accumulated = color;
}