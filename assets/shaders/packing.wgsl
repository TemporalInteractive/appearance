@include ::math

struct PackedRgb9e5 {
    data: u32,
}

struct PackedNormalizedXyz10 {
    data: u32,
}

// Inspired by https://knarkowicz.wordpress.com/2014/04/16/octahedron-normal-vector-encoding/
fn dir_oct_quad_encode(dir: vec3<f32>) -> vec2<f32> {
    var retVal: vec2<f32> = dir.xy / (abs(dir.x) + abs(dir.y) + abs(dir.z));
    if (dir.z < 0.0) {
        var signs = vec2<f32>(select(-1.0, 1.0, retVal.x >= 0.0), select(-1.0, 1.0, retVal.y >= 0.0));
        retVal = (1.0 - abs(retVal.yx)) * signs;
    }
    return retVal * 0.5 + 0.5;
}

// Inspired by https://knarkowicz.wordpress.com/2014/04/16/octahedron-normal-vector-encoding/
fn dir_oct_quad_decode(encoded_in: vec2<f32>) -> vec3<f32> {
    var encoded = encoded_in * 2.0 - 1.0;
    var n = vec3<f32>(encoded.x, encoded.y, 1.0 - abs(encoded.x) - abs(encoded.y));
    var t = saturate(-n.z);
    var added = vec2<f32>(select(t, -t, n.x >= 0.0), select(t, -t, n.y >= 0.0));
    n.x += added.x;
    n.y += added.y;
    return normalize(n);
}

fn pack_30_oct_encoded_dir(oct_encoded_dir: vec2<f32>, offset: u32) -> u32 {
    return ((u32(round(oct_encoded_dir.y * f32(0x7fff))) << 15) |
            u32(round(oct_encoded_dir.x * f32(0x7fff))))
           << offset;
}

fn unpack_30_oct_encoded_dir(packed: u32, offset: u32) -> vec2<f32> {
    return vec2<f32>(f32((packed >> offset) & 0x7fff) / f32(0x7fff),
                  f32((packed >> (offset + 15)) & 0x7fff) / f32(0x7fff));
}

fn PackedNormalizedXyz10::new(data: vec3<f32>, offset: u32) -> PackedNormalizedXyz10 {
    var oct_encoded_dir = dir_oct_quad_encode(data);
    return PackedNormalizedXyz10(pack_30_oct_encoded_dir(oct_encoded_dir, offset));
}

fn PackedNormalizedXyz10::unpack(_self: PackedNormalizedXyz10, offset: u32) -> vec3<f32> {
    return dir_oct_quad_decode(unpack_30_oct_encoded_dir(_self.data, offset));
}

// https://github.com/microsoft/DirectX-Graphics-Samples/blob/master/MiniEngine/Core/Shaders/PixelPacking_RGBE.hlsli
// Copyright (c) Microsoft. All rights reserved.
// This code is licensed under the MIT License (MIT).
fn PackedRgb9e5::new(rgb: vec3<f32>) -> PackedRgb9e5 {
    var max_val: f32 = bitcast<f32>(0x477F8000u);
    var min_val: f32 = bitcast<f32>(0x37800000u);

    var clamped_rgb: vec3<f32> = clamp(rgb, vec3<f32>(0.0), vec3<f32>(max_val));

    var max_channel: f32 = max(max(min_val, clamped_rgb.r), max(clamped_rgb.g, clamped_rgb.b));

    var bias: f32 = bitcast<f32>((bitcast<u32>(max_channel) + 0x07804000u) & 0x7F800000u);

    var rgbui: vec3<u32> = bitcast<vec3<u32>>(clamped_rgb + bias);
    var e: u32 = (bitcast<u32>(bias) << 4u) + 0x10000000u;
    return PackedRgb9e5(e | rgbui.b << 18 | rgbui.g << 9 | (rgbui.r & 0x1FFu));
}

// https://github.com/microsoft/DirectX-Graphics-Samples/blob/master/MiniEngine/Core/Shaders/PixelPacking_RGBE.hlsli
// Copyright (c) Microsoft. All rights reserved.
// This code is licensed under the MIT License (MIT).
fn PackedRgb9e5::unpack(_self: PackedRgb9e5) -> vec3<f32> {
    var rgb: vec3<f32> = vec3<f32>(vec3<u32>(_self.data, _self.data >> 9, _self.data >> 18) & vec3<u32>(0x1FFu));
    return ldexp(rgb, vec3<i32>(_self.data >> 27) - 24);
}