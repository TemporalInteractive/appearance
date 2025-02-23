const RENDER_BLOCK_SIZE: u32 = 64;

fn linear_to_block_pixel_idx(id: vec2<u32>, width: u32) -> vec2<u32> {
    let block_size = vec2<u32>(RENDER_BLOCK_SIZE, RENDER_BLOCK_SIZE);
    
    let block_id: u32 = id / block_size;
    let block_offset: u32 = id % block_size;

    let blocks_per_row: u32 = (width + RENDER_BLOCK_SIZE - 1) / RENDER_BLOCK_SIZE;
    let block_index: u32 = block_id.y * blocks_per_row + block_id.x;
    
    let pixel_index: u32 = block_index * (RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE) +
                      block_offset.y * RENDER_BLOCK_SIZE + block_offset.x;

    return vec2<u32>(pixel_index % width, pixel_index / width);
}