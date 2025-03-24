@include appearance-packing::shared/packing

// Used for sampling lights in the world, can both sample emissive triangles or the sun, indicated by triangle_area = 0
// struct LightSample { // TODO: this will need to be represented as a triangle idx (and some way we can query it's transform), also barycentric coords (usefull for both triangle and sun)
//                      // this will better support moving lights
//     point: vec3<f32>,
//     emission: vec3<f32>,
//     triangle_area: f32, // TODO: remove?
//     triangle_normal: vec3<f32>,
// }

struct LightSampleEvalData {
    emission: vec3<f32>,
    point_ws: vec3<f32>,
}

struct LightSample {
    uv: vec2<f32>,
    emissive_triangle_instance_idx: u32,
    local_triangle_idx: u32,
}

// Packed representation of LightSample
// struct PackedLightSample {
//     point: vec3<f32>,
//     emission: PackedRgb9e5,
//     triangle_area: f32,
//     triangle_normal: PackedNormalizedXyz10,
//     _padding0: u32,
//     _padding1: u32,
// }

struct PackedLightSample {
    uv: vec2<f32>,
    emissive_triangle_instance_idx: u32,
    local_triangle_idx: u32,
}

// Context required to evaluate a light sample
struct LightSampleCtx {
    hit_tex_coord: vec2<f32>,
    hit_material_idx: u32,
    throughput: PackedRgb9e5,
    front_facing_shading_normal_ws: PackedNormalizedXyz10,
    front_facing_clearcoat_normal_ws: PackedNormalizedXyz10,
}

// fn LightSample::new_triangle_sample(point: vec3<f32>, emission: vec3<f32>, triangle: Triangle) -> LightSample {
//     let p01: vec3<f32> = triangle.p1 - triangle.p0;
//     let p02: vec3<f32> = triangle.p2 - triangle.p0;
//     let triangle_area: f32 = Triangle::area_from_edges(p01, p02);
//     let triangle_normal: vec3<f32> = normalize(cross(p01, p02));

//     return LightSample(point, emission, triangle_area, triangle_normal);
// }

fn LightSampleEvalData::new(emission: vec3<f32>, point_ws: vec3<f32>) -> LightSampleEvalData {
    return LightSampleEvalData(emission, point_ws);
}

fn LightSampleEvalData::empty() -> LightSampleEvalData {
    return LightSampleEvalData(vec3<f32>(0.0), vec3<f32>(0.0));
}

fn LightSample::new_triangle_sample(uv: vec2<f32>, emissive_triangle_instance_idx: u32, local_triangle_idx: u32) -> LightSample {
    return LightSample(uv, emissive_triangle_instance_idx, local_triangle_idx);
}

fn LightSample::new_sun_sample(uv: vec2<f32>) -> LightSample {
    return LightSample(uv, U32_MAX, 0);
}

fn LightSample::empty() -> LightSample {
    return LightSample(vec2<f32>(-1.0), 0, 0);
}

fn LightSample::is_empty(_self: LightSample) -> bool {
    return _self.uv.x < -0.0001;
}

fn LightSample::is_sun(_self: LightSample) -> bool {
    return _self.emissive_triangle_instance_idx == U32_MAX;
}

// fn PackedLightSample::new(light_sample: LightSample) -> PackedLightSample {
//     return PackedLightSample(
//         light_sample.point,
//         PackedRgb9e5::new(light_sample.emission),
//         light_sample.triangle_area,
//         PackedNormalizedXyz10::new(light_sample.triangle_normal, 0),
//         0,
//         0
//     );
// }

// fn PackedLightSample::unpack(_self: PackedLightSample) -> LightSample {
//     return LightSample(
//         _self.point,
//         PackedRgb9e5::unpack(_self.emission),
//         _self.triangle_area,
//         PackedNormalizedXyz10::unpack(_self.triangle_normal, 0)
//     );
// }

fn PackedLightSample::new(light_sample: LightSample) -> PackedLightSample {
    return PackedLightSample(
        light_sample.uv,
        light_sample.emissive_triangle_instance_idx,
        light_sample.local_triangle_idx
    );
}

fn PackedLightSample::unpack(_self: PackedLightSample) -> LightSample {
    return LightSample(
        _self.uv,
        _self.emissive_triangle_instance_idx,
        _self.local_triangle_idx
    );
}

fn LightSampleCtx::new(hit_tex_coord: vec2<f32>, hit_material_idx: u32, throughput: vec3<f32>, front_facing_shading_normal_ws: vec3<f32>, front_facing_clearcoat_normal_ws: vec3<f32>) -> LightSampleCtx {
    return LightSampleCtx(
        hit_tex_coord,
        hit_material_idx,
        PackedRgb9e5::new(throughput),
        PackedNormalizedXyz10::new(front_facing_shading_normal_ws, 0),
        PackedNormalizedXyz10::new(front_facing_clearcoat_normal_ws, 0)
    );
}