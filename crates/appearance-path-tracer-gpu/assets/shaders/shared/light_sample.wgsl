@include appearance-packing::shared/packing

// Data required to evaluate a light sample from any given hit position in the world, this must be reevaluated every frame as triangle can move over time & their intensity can change
struct LightSampleEvalData {
    emission: vec3<f32>,
    point_ws: vec3<f32>,
}

// Used for sampling lights in the world, can both sample emissive triangles or the sun
struct LightSample {
    uv: vec2<f32>,
    emissive_triangle_instance_idx: u32,
    local_triangle_idx: u32,
}

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
    return LightSample(uv, U32_MAX - 1, 0);
}

fn LightSample::empty() -> LightSample {
    return LightSample(vec2<f32>(0.0), U32_MAX, 0);
}

fn LightSample::is_empty(_self: LightSample) -> bool {
    return _self.emissive_triangle_instance_idx == U32_MAX;
}

fn LightSample::is_sun(_self: LightSample) -> bool {
    return _self.emissive_triangle_instance_idx == U32_MAX - 1;
}

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