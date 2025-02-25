#include "appearance-render-loop/block.hlsl"
#include "ray.hlsl"

[[vk::binding(0, 0)]]
cbuffer Constants {
    uint2 resolution;
    uint _padding0;
    uint _padding1;
};

[[vk::binding(1, 0)]]
StructuredBuffer<Payload> payloads;

[[vk::binding(2, 0)]]
[[vk::image_format("rgba8")]]
RWTexture2D<unorm float4> texture;

[numthreads(16, 16, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint2 id = global_id.xy;
    if (any(id >= resolution)) return;

    Payload payload = payloads.Load(id.y * resolution.x + id.x);

    uint2 blockId = LinearToBlockPixelIdx(id, resolution.x);
    texture[int2(blockId)] = float4(payload.accumulated, 1.0);
}