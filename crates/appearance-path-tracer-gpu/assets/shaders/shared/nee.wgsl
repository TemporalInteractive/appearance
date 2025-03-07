@include ::triangle
@include ::packing

///
/// BINDING DEPENDENCIES:
/// appearance-path-tracer-gpu::shared/vertex_pool_bindings
/// appearance-path-tracer-gpu::shared/material_pool_bindings
/// appearance-path-tracer-gpu::shared/sky_bindings
///

struct LightSample {
    direction: vec3<f32>,
    distance: f32,
    pdf: f32,
    emission: vec3<f32>,
    triangle_area: f32,
    triangle_normal: vec3<f32>,
}

struct PackedLightSample {
    direction: PackedNormalizedXyz10,
    distance: f32,
    pdf: f32,
    emission: PackedRgb9e5,
    triangle_area: f32,
    triangle_normal: PackedNormalizedXyz10,
}

fn LightSample::new_triangle_sample(direction: vec3<f32>, distance: f32, pdf: f32, emission: vec3<f32>, triangle: Triangle) -> LightSample {
    let p01: vec3<f32> = triangle.p1 - triangle.p0;
    let p02: vec3<f32> = triangle.p2 - triangle.p0;
    let triangle_area: f32 = Triangle::area_from_edges(p01, p02);
    let triangle_normal: vec3<f32> = normalize(cross(p01, p02));

    return LightSample(direction, distance, pdf, emission, triangle_area, triangle_normal);
}

fn LightSample::new_sun_sample(direction: vec3<f32>, distance: f32, pdf: f32, emission: vec3<f32>) -> LightSample {
    return LightSample(direction, distance, pdf, emission, 0.0, UP);
}

fn LightSample::empty() -> LightSample {
    return LightSample(vec3<f32>(0.0), 0.0, 0.0, vec3<f32>(0.0), 0.0, UP);
}

fn LightSample::intensity(_self: LightSample) -> f32 {
    if (_self.triangle_area == 0.0) {
        return Sky::sun_intensity(_self.direction.y);
    } else {
        let cos_out: f32 = abs(dot(_self.triangle_normal, -_self.direction));

        return Triangle::solid_angle(cos_out, _self.triangle_area, _self.distance) * 10.0;
    }
}

fn PackedLightSample::new(light_sample: LightSample) -> PackedLightSample {
    return PackedLightSample(
        PackedNormalizedXyz10::new(light_sample.direction, 0),
        light_sample.distance,
        light_sample.pdf,
        PackedRgb9e5::new(light_sample.emission),
        light_sample.triangle_area,
        PackedNormalizedXyz10::new(light_sample.triangle_normal, 0)
    );
}

fn PackedLightSample::unpack(_self: PackedLightSample) -> LightSample {
    return LightSample(
        PackedNormalizedXyz10::unpack(_self.direction, 0),
        _self.distance,
        _self.pdf,
        PackedRgb9e5::unpack(_self.emission),
        _self.triangle_area,
        PackedNormalizedXyz10::unpack(_self.triangle_normal, 0)
    );
}

fn Nee::sample_emissive_triangle(r0: f32, r1: f32, r23: vec2<f32>, sample_point: vec3<f32>, sun_pick_probability: f32) -> LightSample {
    for (var i: u32 = 0; i < vertex_pool_constants.num_emissive_triangle_instances; i += 1) {
        let emissive_triangle_instance: EmissiveTriangleInstance = emissive_triangle_instances[i];
        if (r0 <= emissive_triangle_instance.cdf) {
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[emissive_triangle_instance.vertex_pool_slice_idx];

            let local_triangle_idx: u32 = u32(r1 * f32(emissive_triangle_instance.num_triangles));
            let first_index: u32 = vertex_pool_slice.first_index + (local_triangle_idx * 3);

            let i0: u32 = vertex_indices[first_index + 0];
            let i1: u32 = vertex_indices[first_index + 1];
            let i2: u32 = vertex_indices[first_index + 2];

            let barycentrics = vec3<f32>(1.0 - r23.x - r23.y, r23);

            let tex_coord0: vec2<f32> = vertex_tex_coords[vertex_pool_slice.first_vertex + i0];
            let tex_coord1: vec2<f32> = vertex_tex_coords[vertex_pool_slice.first_vertex + i1];
            let tex_coord2: vec2<f32> = vertex_tex_coords[vertex_pool_slice.first_vertex + i2];
            let tex_coord: vec2<f32> = tex_coord0 * barycentrics.x + tex_coord1 * barycentrics.y + tex_coord2 * barycentrics.z;

            let position0: vec3<f32> = vertex_positions[vertex_pool_slice.first_vertex + i0].xyz;
            let position1: vec3<f32> = vertex_positions[vertex_pool_slice.first_vertex + i1].xyz;
            let position2: vec3<f32> = vertex_positions[vertex_pool_slice.first_vertex + i2].xyz;
            var triangle = Triangle::new(position0, position1, position2);
            triangle = Triangle::transform(triangle, emissive_triangle_instance.transform);
            let position: vec3<f32> = triangle.p0 * barycentrics.x + triangle.p1 * barycentrics.y + triangle.p2 * barycentrics.z;

            // var direction: vec3<f32> = sample_point - position;
            // let distance: f32 = length(direction);
            // direction /= distance;

            let direction: vec3<f32> = normalize(position - sample_point);
            let distance: f32 = distance(sample_point, position);

            let pdf: f32 = max(1e-6, (1.0 - sun_pick_probability) / f32(vertex_pool_constants.num_emissive_triangles));

            let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + local_triangle_idx];
            let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
            let emission: vec3<f32> = MaterialDescriptor::emission(material_descriptor, tex_coord);

            return LightSample::new_triangle_sample(direction, distance, pdf, emission, triangle);
        }
    }

    return LightSample::empty();
}

fn Nee::sample_sun(r01: vec2<f32>, sun_pick_probability: f32) -> LightSample {
    let direction: vec3<f32> =  Sky::direction_to_sun(r01);
    let distance: f32 = 10000.0;
    let pdf: f32 = sun_pick_probability;
    let emission = sky_constants.sun_color;
    return LightSample::new_sun_sample(direction, distance, pdf, emission);
}

fn Nee::sample(r0: f32, r1: f32, r2: f32, r34: vec2<f32>, sample_point: vec3<f32>) -> LightSample {
    var sun_pick_probability: f32;
    if (vertex_pool_constants.num_emissive_triangles > 0) {
        sun_pick_probability = 0.5;
    } else {
        sun_pick_probability = 1.0;
    }

    if (r0 < sun_pick_probability) {
        return Nee::sample_sun(r34, sun_pick_probability);
    } else {
        return Nee::sample_emissive_triangle(r1, r2, r34, sample_point, sun_pick_probability);
    }
}