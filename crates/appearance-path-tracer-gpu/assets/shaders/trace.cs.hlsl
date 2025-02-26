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

[[vk::binding(3, 0)]]
RaytracingAccelerationStructure scene;

[numthreads(128, 1, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint id = global_id.x;
    if (id >= rayCount) return;
    
    Ray ray = rays[id];
    
    RayDesc rayDesc = (RayDesc)0;
    rayDesc.TMin = 0.0;
    rayDesc.Direction = ray.direction;
    rayDesc.TMax = 1e9f;
    rayDesc.Origin = ray.origin;

    const RAY_FLAG flags = RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES;
    RayQuery<flags> q;
    q.TraceRayInline(scene, flags,
                     0xFF, rayDesc);
    while (q.Proceed()) {}

    float3 color = (float3)0;
    if (q.CommittedStatus() == COMMITTED_TRIANGLE_HIT) {
        color = float3(0.0, 1.0, 0.5);
    } else {
        float a = 0.5 * (ray.direction.y + 1.0);
        color = (1.0 - a) * float3(1.0, 1.0, 1.0) + a * float3(0.5, 0.7, 1.0);
    }

    payloads[id].accumulated = color;
}