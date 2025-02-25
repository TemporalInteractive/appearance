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