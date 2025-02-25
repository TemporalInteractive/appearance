#include "appearance-wgpu/bindless.hlsl"
#include "appearance-render-loop/block.hlsl"

[[vk::binding(0, 1)]]
cbuffer Bindings {
    uint width;
    uint height;
    BindlessBuffer clearColorBuffer;
    uint _padding0;
    //uint _padding1;
};

[[vk::binding(1, 1)]]
[[vk::image_format("rgba8")]]
RWTexture2D<unorm float4> texture;

[numthreads(16, 16, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint2 id = global_id.xy;
    if (id.x >= width || id.y >= height) return;

    float2 pixelCenter = float2(float(id.x) + 0.5, float(id.y) + 0.5);
    float2 uv = pixelCenter / float2(float(width), float(height));

    float4 color = clearColorBuffer.Load(0);
    //float4 color = float4(1.0, 0.0, 0.0, 0.0);//.Load<float4>(0);
    
    uint2 blockId = LinearToBlockPixelIdx(id, width);
    texture[int2(blockId)] = float4(color.rgb, 1.0);
}