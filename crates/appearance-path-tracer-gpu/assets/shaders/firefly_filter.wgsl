@include ::color
@include ::math

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    resolution: vec2<u32>,
    _padding0: u32,
    _padding1: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read_write> demodulated_radiance: array<PackedRgb9e5>;

@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(num_workgroups) dispatch_size: vec3<u32>) {
    let id: vec2<u32> = global_id.xy;
    if (any(id >= constants.resolution)) { return; }
    let flat_id: u32 = id.y * constants.resolution.x + id.x;

    let current_gbuffer_texel: GBufferTexel = gbuffer[flat_id];

    if (GBufferTexel::is_sky(current_gbuffer_texel)) {
        return;
    }

    var curr_color: vec3<f32> = PackedRgb9e5::unpack(demodulated_radiance[flat_id]);

    var min_lum: f32 = F32_MAX;
    var max_lum: f32 = 0.0;
    var min_color: vec3<f32> = curr_color;
    var max_color = vec3<f32>(0.0);
    var curr_lum: f32 = linear_to_luma(curr_color);

    for (var x: i32 = -1; x <= 1; x += 1) {
        for (var y: i32 = -1; y <= 1; y += 1) {
            if (x == 0 && y == 0) {
                continue;
            }

            let sample_pixel: vec2<i32> = vec2<i32>(id) + vec2<i32>(x, y);
            if (any(sample_pixel < vec2<i32>(0)) || any(sample_pixel >= vec2<i32>(constants.resolution))) {
                continue;
            }
            let flat_sample_pixel: u32 = u32(sample_pixel.y) * constants.resolution.x + u32(sample_pixel.x);

            let neighbor_gbuffer_texel: GBufferTexel = gbuffer[flat_sample_pixel];
            if (GBufferTexel::is_sky(neighbor_gbuffer_texel)) {
                continue;
            }

            let neighbor_color: vec3<f32> = PackedRgb9e5::unpack(demodulated_radiance[flat_sample_pixel]);
            let neighbor_lum: f32 = linear_to_luma(neighbor_color);

            if (neighbor_lum < min_lum) {
                min_lum = neighbor_lum;
                min_color = neighbor_color;
            } else if (neighbor_lum > max_lum) {
                max_lum = neighbor_lum;
                max_color = neighbor_color;
            }
        }
    }

    var reconstructed: vec3<f32>;
    if (curr_lum < min_lum) {
        reconstructed = min_color;
    } else if (curr_lum > max_lum) {
        reconstructed = max_color;
    } else {
        reconstructed = curr_color;
    }
    if (min_lum > max_lum) {
        reconstructed = curr_color;
    }

    demodulated_radiance[flat_id] = PackedRgb9e5::new(reconstructed);
}