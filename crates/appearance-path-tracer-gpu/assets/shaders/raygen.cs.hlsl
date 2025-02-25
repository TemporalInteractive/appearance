#include "appearance-render-loop/block.hlsl"

[[vk::binding(0, 0)]]
cbuffer Constants {
    uint width;
    uint height;
    uint _padding0;
    uint _padding1;
};

[[vk::binding(1, 0)]]
[[vk::image_format("rgba8")]]
RWTexture2D<unorm float4> texture;

[numthreads(16, 16, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint2 id = global_id.xy;
    if (id.x >= width || id.y >= height) return;

    float2 pixelCenter = float2(float(id.x) + 0.5, float(id.y) + 0.5);
    float2 uv = pixelCenter / float2(float(width), float(height));
    
    uint2 blockId = LinearToBlockPixelIdx(id, width);
    texture[int2(blockId)] = float4(uv.x, uv.y, 0.0, 1.0);
}