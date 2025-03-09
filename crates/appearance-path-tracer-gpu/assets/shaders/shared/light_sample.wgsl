@include ::triangle
@include ::packing

// Used for sampling lights in the world, can both sample emissive triangles or the sun, indicated by triangle_area = 0
struct LightSample {
    point: vec3<f32>,
    emission: vec3<f32>,
    triangle_area: f32,
    triangle_normal: vec3<f32>,
}

// Packed representation of LightSample
struct PackedLightSample {
    point: vec3<f32>,
    emission: PackedRgb9e5,
    triangle_area: f32,
    triangle_normal: PackedNormalizedXyz10,
    _padding0: u32,
    _padding1: u32,
}

// Context required to evaluate a light sample
struct LightSampleCtx {
    hit_tex_coord: vec2<f32>,
    hit_material_idx: u32,
    throughput: PackedRgb9e5,
    front_facing_shading_normal_ws: PackedNormalizedXyz10,
    front_facing_clearcoat_normal_ws: PackedNormalizedXyz10,
}

fn LightSample::new_triangle_sample(point: vec3<f32>, emission: vec3<f32>, triangle: Triangle) -> LightSample {
    let p01: vec3<f32> = triangle.p1 - triangle.p0;
    let p02: vec3<f32> = triangle.p2 - triangle.p0;
    let triangle_area: f32 = Triangle::area_from_edges(p01, p02);
    let triangle_normal: vec3<f32> = normalize(cross(p01, p02));

    return LightSample(point, emission, triangle_area, triangle_normal);
}

fn LightSample::new_sun_sample(point: vec3<f32>, emission: vec3<f32>) -> LightSample {
    return LightSample(point, emission, 0.0, UP);
}

fn LightSample::empty() -> LightSample {
    return LightSample(vec3<f32>(0.0), vec3<f32>(0.0), 0.0, UP);
}

fn PackedLightSample::new(light_sample: LightSample) -> PackedLightSample {
    return PackedLightSample(
        light_sample.point,
        PackedRgb9e5::new(light_sample.emission),
        light_sample.triangle_area,
        PackedNormalizedXyz10::new(light_sample.triangle_normal, 0),
        0,
        0
    );
}

fn PackedLightSample::unpack(_self: PackedLightSample) -> LightSample {
    return LightSample(
        _self.point,
        PackedRgb9e5::unpack(_self.emission),
        _self.triangle_area,
        PackedNormalizedXyz10::unpack(_self.triangle_normal, 0)
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