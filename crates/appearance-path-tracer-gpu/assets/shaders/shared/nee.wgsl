@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/restir_di/di_reservoir
@include appearance-path-tracer-gpu::shared/material/disney_bsdf

///
/// BINDING DEPENDENCIES:
/// appearance-path-tracer-gpu::shared/vertex_pool_bindings
/// appearance-path-tracer-gpu::shared/material/material_pool_bindings
/// appearance-path-tracer-gpu::shared/sky_bindings
///

fn LightSample::intensity(_self: LightSample, hit_point_ws: vec3<f32>) -> f32 {
    let direction: vec3<f32> = normalize(_self.point - hit_point_ws);
    let distance: f32 = distance(_self.point, hit_point_ws);

    if (_self.triangle_area == 0.0) {
        return Sky::sun_intensity(direction.y);
    } else {
        let cos_out: f32 = abs(dot(_self.triangle_normal, -direction));

        return Triangle::solid_angle(cos_out, _self.triangle_area, distance) * 10.0;
    }
}

fn Nee::sample_emissive_triangle(r0: f32, r1: f32, r23: vec2<f32>, sample_point: vec3<f32>, sun_pick_probability: f32, pdf: ptr<function, f32>) -> LightSample {
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
            let point: vec3<f32> = triangle.p0 * barycentrics.x + triangle.p1 * barycentrics.y + triangle.p2 * barycentrics.z;

            // var direction: vec3<f32> = sample_point - position;
            // let distance: f32 = length(direction);
            // direction /= distance;

            // let direction: vec3<f32> = normalize(position - sample_point);
            // let distance: f32 = distance(sample_point, position);

            *pdf = max(1e-6, (1.0 - sun_pick_probability) / f32(vertex_pool_constants.num_emissive_triangles));

            let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + local_triangle_idx];
            let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
            let emission: vec3<f32> = MaterialDescriptor::emission(material_descriptor, tex_coord);

            return LightSample::new_triangle_sample(point, emission, triangle);
        }
    }

    *pdf = 0.0;
    return LightSample::empty();
}

fn Nee::sample_sun(r01: vec2<f32>, sun_pick_probability: f32, pdf: ptr<function, f32>) -> LightSample {
    let direction: vec3<f32> =  Sky::direction_to_sun(r01);
    let distance: f32 = 1000000.0;
    *pdf = sun_pick_probability;
    let emission = sky_constants.sun_color;
    let point: vec3<f32> = direction * distance;

    return LightSample::new_sun_sample(point, emission);
}

fn Nee::sample_uniform(r0: f32, r1: f32, r2: f32, r34: vec2<f32>, sample_point: vec3<f32>, pdf: ptr<function, f32>) -> LightSample {
    var sun_pick_probability: f32;
    if (vertex_pool_constants.num_emissive_triangles > 0) {
        sun_pick_probability = 0.5;
    } else {
        sun_pick_probability = 1.0;
    }

    sun_pick_probability = 0.0; // TODO: temp

    if (r0 < sun_pick_probability) {
        return Nee::sample_sun(r34, sun_pick_probability, pdf);
    } else {
        return Nee::sample_emissive_triangle(r1, r2, r34, sample_point, sun_pick_probability, pdf);
    }
}

fn Nee::sample_ris(hit_point_ws: vec3<f32>, w_out_worldspace: vec3<f32>, front_facing_shading_normal_ws: vec3<f32>,
     tangent_to_world: mat3x3<f32>, world_to_tangent: mat3x3<f32>, clearcoat_tangent_to_world: mat3x3<f32>, clearcoat_world_to_tangent: mat3x3<f32>,
     disney_bsdf: DisneyBsdf, rng: ptr<function, u32>, scene: acceleration_structure) -> DiReservoir {
    const NUM_SAMPLES: u32 = 4;

    var di_reservoir = DiReservoir::new();

    for (var i: u32 = 0; i < NUM_SAMPLES; i += 1) {
        var sample_pdf: f32;
        let sample: LightSample = Nee::sample_uniform(random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
            vec2<f32>(random_uniform_float(rng), random_uniform_float(rng)), hit_point_ws, &sample_pdf);

        let w_in_worldspace: vec3<f32> = normalize(sample.point - hit_point_ws);

        let n_dot_l: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);
        if (n_dot_l > 0.0) {
            let sample_intensity = LightSample::intensity(sample, hit_point_ws);

            // TODO: a cheaper approximation of the disney bsdf is desirable here
            var shading_pdf: f32;
            let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
                tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, w_in_worldspace, &shading_pdf);
            let contribution: vec3<f32> = n_dot_l * reflectance;

            let phat: f32 = length(contribution * sample_intensity);
            let weight: f32 = phat / sample_pdf;
            DiReservoir::update(&di_reservoir, weight, rng, sample, phat);
        } else {
            di_reservoir.sample_count += 1;
        }
    }

    if (di_reservoir.selected_phat > 0.0) {
        di_reservoir.contribution_weight = (1.0 / di_reservoir.selected_phat) * (1.0 / di_reservoir.sample_count * di_reservoir.weight_sum);

        let direction: vec3<f32> = normalize(di_reservoir.sample.point - hit_point_ws);
        let distance: f32 = distance(di_reservoir.sample.point, hit_point_ws);

        if (!trace_shadow_ray(hit_point_ws, direction, distance, scene)) {
            di_reservoir.contribution_weight = 0.0;
        }
    }

    return di_reservoir;
}