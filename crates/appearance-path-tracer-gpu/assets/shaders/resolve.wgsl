@include appearance-render-loop::block
@include appearance-path-tracer-gpu::shared/ray

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    width: u32,
    height: u32,
    sample_count: u32,
    accum_frame_count: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read> radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read_write> accum_radiance: array<vec4<f32>>;

@group(0)
@binding(3)
var texture: texture_storage_2d<rgba8unorm, read_write>;

fn hdr_to_sdr(hdr: vec3<f32>) -> vec3<f32> {
    let a: f32 = 2.51;
    let b: f32 = 0.03;
    let c: f32 = 2.43;
    let d: f32 = 0.59;
    let e: f32 = 0.14;
    
    let sdr: vec3<f32> = (hdr * (a * hdr + b)) / (hdr * (c * hdr + d) + e);
    return clamp(sdr, vec3<f32>(0.0), vec3<f32>(1.0));
}

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    var id: vec2<u32> = global_id.xy;
    if (id.x >= constants.width || id.y >= constants.height) { return; }
    var i: u32 = id.y * constants.width + id.x;

    var radiance: vec3<f32> = PackedRgb9e5::unpack(radiance[i]);
    radiance /= f32(constants.sample_count);
    radiance = hdr_to_sdr(radiance);

    var accumulated_radiance: vec3<f32> = accum_radiance[i].rgb;
    accumulated_radiance += radiance;
    accum_radiance[i] = vec4<f32>(accumulated_radiance, 0.0);

    let block_id: vec2<u32> = linear_to_block_pixel_idx(id, constants.width);
    textureStore(texture, vec2(i32(block_id.x), i32(block_id.y)), vec4(accumulated_radiance / f32(constants.accum_frame_count + 1), 1.0));
    //textureStore(texture, vec2(i32(block_id.x), i32(block_id.y)), vec4(radiance, 1.0));
}