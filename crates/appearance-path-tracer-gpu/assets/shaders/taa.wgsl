@include ::random
@include ::color
@include ::math

@include appearance-path-tracer-gpu::shared/gbuffer_bindings

struct Constants {
    resolution: vec2<u32>,
    history_influence: f32,
    _padding0: u32,
}

@group(0)
@binding(0)
var<uniform> constants: Constants;

@group(0)
@binding(1)
var<storage, read_write> demodulated_radiance: array<PackedRgb9e5>;

@group(0)
@binding(2)
var<storage, read> prev_demodulated_radiance: array<PackedRgb9e5>;

// Source: M. Pharr, W. Jakob, and G. Humphreys, Physically Based Rendering, Morgan Kaufmann, 2016.
fn mitchell1D(_x: f32, B: f32, C: f32) -> f32
{
    let x: f32 = abs(2.0 * _x);
    let oneDivSix: f32 = 1.0 / 6.0;

    if (x > 1)
    {
        return ((-B - 6.0 * C) * x * x * x + (6.0 * B + 30.0 * C) * x * x +
                (-12.0 * B - 48.0 * C) * x + (8.0 * B + 24.0 * C)) * oneDivSix;
    }
    else
    {
        return ((12.0 - 9.0 * B - 6.0 * C) * x * x * x +
                (-18.0 + 12.0 * B + 6.0 * C) * x * x +
                (6.0 - 2.0 * B)) * oneDivSix;
    }
}

// Source: https://github.com/playdeadgames/temporal
fn clipAabb(aabbMin: vec3<f32>, aabbMax: vec3<f32>, histSample: vec3<f32>) -> vec3<f32>
{
    let center: vec3<f32> = 0.5 * (aabbMax + aabbMin);
    let extents: vec3<f32> = 0.5 * (aabbMax - aabbMin);

    let rayToCenter: vec3<f32> = histSample - center;
    var rayToCenterUnit: vec3<f32> = rayToCenter.xyz / extents;
    rayToCenterUnit = abs(rayToCenterUnit);
    let rayToCenterUnitMax: f32 = max(rayToCenterUnit.x, max(rayToCenterUnit.y, rayToCenterUnit.z));

    if (rayToCenterUnitMax > 1.0)
    {
        return center + rayToCenter / rayToCenterUnitMax;
    }
    else
    {
        return histSample;
    }
}

fn bilinear(tx: f32, ty: f32, c00: vec3<f32>, c10: vec3<f32>, c01: vec3<f32>, c11: vec3<f32>) -> vec3<f32> {
    var a: vec3<f32> = c00 * (1.0 - tx) + c10 * tx;
    var b: vec3<f32> = c01 * (1.0 - tx) + c11 * tx;
    return a * (1.0 - ty) + b * ty;
}

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

    var current: vec3<f32> = PackedRgb9e5::unpack(demodulated_radiance[flat_id]);

    var weightSum: f32 = sqr(mitchell1D(0, 0.33, 0.33));
    var reconstructed: vec3<f32> = current * weightSum;
    var firstMoment: vec3<f32> = current;
    var secondMoment: vec3<f32>  = current * current;

    var sampleCount: f32 = 1.0;

    for (var x: i32 = -1; x <= 1; x += 1)
    {
        for (var y: i32 = -1; y <= 1; y += 1)
        {
            if (x == 0 && y == 0)
            {
                continue;
            }

            let samplePixel: vec2<i32> = vec2<i32>(id) + vec2<i32>(x, y);
            if (any(samplePixel < vec2<i32>(0)) || any(samplePixel >= vec2<i32>(constants.resolution - 1)))
            {
                continue;
            }

            let flat_sample_pixel: u32 = u32(samplePixel.y) * constants.resolution.x + u32(samplePixel.x);

            let sampleColor: vec3<f32> = max(PackedRgb9e5::unpack(demodulated_radiance[flat_sample_pixel]), vec3<f32>(0.0)); // TODO: clamp required?
            var weight: f32 = mitchell1D(f32(x), 0.33, 0.33) * mitchell1D(f32(y), 0.33, 0.33);
            weight *= 1.0 / (1.0 + linear_to_luma(sampleColor));

            reconstructed += sampleColor * weight;
            weightSum += weight;

            firstMoment += sampleColor;
            secondMoment += sampleColor * sampleColor;

            sampleCount += 1.0;
        }
    }

    reconstructed /= max(weightSum, 1e-5);

    var history: vec3<f32>;
    var prev_point_ss: vec2<f32>;
    if (GBuffer::reproject(current_gbuffer_texel.position_ws, constants.resolution, &prev_point_ss)) {
        //let prev_id_2d = vec2<u32>(floor(prev_point_ss) - 1);

        prev_point_ss -= 0.5;

        let prev_id00: u32 = min(u32(floor(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(floor(prev_point_ss.x)), constants.resolution.x - 1);
        let history00: vec3<f32> = PackedRgb9e5::unpack(prev_demodulated_radiance[prev_id00]);
        let prev_id10: u32 = min(u32(ceil(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(floor(prev_point_ss.x)), constants.resolution.x - 1);
        let history10: vec3<f32> = PackedRgb9e5::unpack(prev_demodulated_radiance[prev_id10]);
        let prev_id01: u32 = min(u32(floor(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(ceil(prev_point_ss.x)), constants.resolution.x - 1);
        let history01: vec3<f32> = PackedRgb9e5::unpack(prev_demodulated_radiance[prev_id01]);
        let prev_id11: u32 = min(u32(ceil(prev_point_ss.y)), constants.resolution.y - 1) * constants.resolution.x + min(u32(ceil(prev_point_ss.x)), constants.resolution.x - 1);
        let history11: vec3<f32> = PackedRgb9e5::unpack(prev_demodulated_radiance[prev_id11]);

        history = bilinear(fract(prev_point_ss.x), fract(prev_point_ss.y), history00, history10, history01, history11);
    } else {
        history = PackedRgb9e5::unpack(prev_demodulated_radiance[flat_id]);
    }

    // let prev_gbuffer_texel: GBufferTexel = prev_gbuffer[prev_id];
    // let current_depth_cs: f32 = GBufferTexel::depth_cs(current_gbuffer_texel, 0.001, 10000.0);
    // let prev_depth_cs: f32 = GBufferTexel::depth_cs(prev_gbuffer_texel, 0.001, 10000.0);
    let valid_delta_depth: bool = true;//(abs(current_depth_cs - prev_depth_cs) / current_depth_cs) < 0.2;
    // let current_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(current_gbuffer_texel.normal_ws, 0);
    // let prev_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(prev_gbuffer_texel.normal_ws, 0);
    // let valid_delta_normal: bool = dot(current_normal_ws, prev_normal_ws) > 0.906; // 25 degrees
    
    let mean: vec3<f32> = firstMoment / sampleCount;
    var stdev: vec3<f32> = abs(secondMoment - (firstMoment * firstMoment) / sampleCount);
    stdev /= (sampleCount - 1.0);
    stdev = sqrt(stdev);

    let clippedHistory: vec3<f32> = clipAabb(mean - stdev, mean + stdev, history);

    var historyWeightFactor: f32;
    if (!valid_delta_depth) {
        historyWeightFactor = 1.0;
    } else {
        historyWeightFactor = 1.0;
    }
    let blendWeight: f32 = 1.0 - (constants.history_influence * historyWeightFactor);

    let currentWeight: f32 = saturate(blendWeight * (1.0 / (1.0 + linear_to_luma(reconstructed))));
    let historyWeight: f32 = saturate((1.0 - blendWeight) * (1.0 / (1.0 + linear_to_luma(clippedHistory))));
    reconstructed = (currentWeight * reconstructed + historyWeight * clippedHistory) / (currentWeight + historyWeight);

    demodulated_radiance[flat_id] = PackedRgb9e5::new(reconstructed);
}