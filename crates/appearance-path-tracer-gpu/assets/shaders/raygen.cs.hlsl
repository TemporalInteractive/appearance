#define RENDER_BLOCK_SIZE 64

uint2 LinearToBlockPixelIdx(uint2 id, uint width) {
    uint2 blockSize = uint2(RENDER_BLOCK_SIZE, RENDER_BLOCK_SIZE);
    
    uint2 blockId = id / blockSize;
    uint2 blockOffset = id % blockSize;

    uint blocksPerRow = (width + RENDER_BLOCK_SIZE - 1) / RENDER_BLOCK_SIZE;
    uint blockIndex = blockId.y * blocksPerRow + blockId.x;
    
    uint pixelIndex = blockIndex * (RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE) +
                      blockOffset.y * RENDER_BLOCK_SIZE + blockOffset.x;

    return uint2(pixelIndex % width, pixelIndex / width);
}

[[vk::binding(0, 0)]]
cbuffer Constants {
    uint width;
    uint height;
    uint _padding0;
    uint _padding1;
};
//StructuredBuffer<uint> constants;

[[vk::binding(1, 0)]]
[[vk::image_format("rgba8")]]
RWTexture2D<unorm float4> texture;

[numthreads(16, 16, 1)]
void main(uint3 global_id : SV_DispatchThreadID) {
    uint2 id = global_id.xy;
    // uint width = constants[0];
    // uint height = constants[1];

    if (id.x >= width || id.y >= height) return;

    float2 pixelCenter = float2(float(id.x) + 0.5, float(id.y) + 0.5);
    float2 uv = pixelCenter / float2(float(width), float(height));
    
    uint2 blockId = LinearToBlockPixelIdx(id, width);
    texture[int2(blockId)] = float4(uv.x, uv.y, 0.0, 1.0);
}