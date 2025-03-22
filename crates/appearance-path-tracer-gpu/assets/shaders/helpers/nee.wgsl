@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/restir/di_reservoir
@include appearance-path-tracer-gpu::shared/material/disney_bsdf

@include appearance-path-tracer-gpu::helpers/trace

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
        return Sky::sun_intensity(direction);
    } else {
        return Triangle::solid_angle(_self.triangle_normal, direction, distance) * 10.0;
    }
}

fn Nee::sample_emissive_triangle(r0: f32, r1: f32, r23: vec2<f32>, sample_point: vec3<f32>, sun_pick_probability: f32, pdf: ptr<function, f32>) -> LightSample {
    for (var i: u32 = 0; i < vertex_pool_constants.num_emissive_triangle_instances; i += 1) {
        let emissive_triangle_instance: EmissiveTriangleInstance = emissive_triangle_instances[i]; // TODO: speedup
        if (r0 <= emissive_triangle_instance.cdf) {
            let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[emissive_triangle_instance.vertex_pool_slice_idx];

            let local_triangle_idx: u32 = u32(r1 * f32(emissive_triangle_instance.num_triangles));
            let first_index: u32 = vertex_pool_slice.first_index + (local_triangle_idx * 3);

            let i0: u32 = vertex_indices[first_index + 0];
            let i1: u32 = vertex_indices[first_index + 1];
            let i2: u32 = vertex_indices[first_index + 2];

            let barycentrics = vec3<f32>(1.0 - r23.x - r23.y, r23);

            let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
            let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
            let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

            let tex_coord: vec2<f32> = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;

            var triangle = Triangle::new(v0.position, v1.position, v2.position);
            triangle = Triangle::transform(triangle, emissive_triangle_instance.transform);
            let point: vec3<f32> = triangle.p0 * barycentrics.x + triangle.p1 * barycentrics.y + triangle.p2 * barycentrics.z;

            let p01: vec3<f32> = triangle.p1 - triangle.p0;
            let p02: vec3<f32> = triangle.p2 - triangle.p0;
            let triangle_area: f32 = Triangle::area_from_edges(p01, p02);

            *pdf = 1.0 / f32(vertex_pool_constants.num_emissive_triangles);
            *pdf /= triangle_area;
            *pdf = max(1e-6, (*pdf) * (1.0 - sun_pick_probability));

            let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + local_triangle_idx];
            let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
            let emission: vec3<f32> = MaterialDescriptor::emission(material_descriptor, tex_coord);

            return LightSample::new_triangle_sample(point, emission, triangle);
        }
    }

    // TODO: double check if this every happens, it should never!
    *pdf = 0.0;
    return LightSample::empty();
}

fn Nee::sample_sun(r01: vec2<f32>, sun_pick_probability: f32, pdf: ptr<function, f32>) -> LightSample {
    let direction: vec3<f32> =  Sky::direction_to_sun(r01);
    *pdf = sun_pick_probability;
    let emission = sky_constants.sun_color;
    let point: vec3<f32> = direction * SUN_DISTANCE;

    return LightSample::new_sun_sample(point, emission);
}

fn Nee::sample_uniform(r0: f32, r1: f32, r2: f32, r34: vec2<f32>, sample_point: vec3<f32>, pdf: ptr<function, f32>) -> LightSample {
    var sun_pick_probability: f32;
    if (vertex_pool_constants.num_emissive_triangles > 0) {
        sun_pick_probability = 0.5;
    } else {
        sun_pick_probability = 1.0;
    }

    if (r0 < sun_pick_probability) {
        return Nee::sample_sun(r34, sun_pick_probability, pdf);
    } else {
        return Nee::sample_emissive_triangle(r1, r2, r34, sample_point, sun_pick_probability, pdf);
    }
}

// 𝑚𝑖(𝑥) = 𝑝1(𝑥) / (𝑀1𝑝1(𝑥) + 𝑀2𝑝2(𝑥))
fn balance_heuristic(pdf1: f32, sample_count1: f32, pdf2: f32, sample_count2: f32) -> f32 {
    return pdf1 / (sample_count1 * pdf1 + sample_count2 * pdf2);
}

fn Nee::sample_ris(hit_point_ws: vec3<f32>, w_out_worldspace: vec3<f32>, front_facing_shading_normal_ws: vec3<f32>,
     tangent_to_world: mat3x3<f32>, world_to_tangent: mat3x3<f32>, clearcoat_tangent_to_world: mat3x3<f32>, clearcoat_world_to_tangent: mat3x3<f32>,
     disney_bsdf: DisneyBsdf, t: f32, back_face: bool, rng: ptr<function, u32>, scene: acceleration_structure) -> DiReservoir {
    // var bsdf_sample_pdf: f32 = 0.0;
    // var bsdf_light_sample = LightSample::empty();
    // var bsdf_phat: f32 = 0.0;
    // var w_in_worldspace: vec3<f32>;
    // var specular: bool;
    // let reflectance: vec3<f32> = DisneyBsdf::sample(disney_bsdf,
    //     front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
    //     w_out_worldspace, t, back_face,
    //     random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
    //     &w_in_worldspace, &bsdf_sample_pdf, &specular
    // );

    // let wi_dot_n: f32 = abs(dot(w_in_worldspace, front_facing_shading_normal_ws));
    // let contribution: vec3<f32> = wi_dot_n * reflectance;

    // if (dot(contribution, contribution) > 0.0) {
    //     // TODO: non-opaques
    //     var rq: ray_query;
    //     rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, 1000.0, safe_origin(hit_point_ws, front_facing_shading_normal_ws), w_in_worldspace));
    //     rayQueryProceed(&rq);
    //     let intersection = rayQueryGetCommittedIntersection(&rq);
    //     if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
    //         let vertex_pool_slice_index: u32 = intersection.instance_custom_data;
    //         let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[vertex_pool_slice_index];

    //         let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

    //         let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
    //         let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
    //         let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

    //         let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
    //         let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
    //         let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

    //         let tex_coord: vec2<f32> = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;

    //         let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + intersection.primitive_index];
    //         let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
    //         let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord);
    //         if (dot(material.emission, material.emission) > 0.0) {
    //             var triangle = Triangle::new(
    //                 (intersection.object_to_world * vec4<f32>(v0.position, 1.0)).xyz,
    //                 (intersection.object_to_world * vec4<f32>(v1.position, 1.0)).xyz,
    //                 (intersection.object_to_world * vec4<f32>(v2.position, 1.0)).xyz
    //             );
    //             let point: vec3<f32> = hit_point_ws + w_in_worldspace * safely_traced_t(intersection.t - 0.01);

    //             //let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);
    //             //let point: vec3<f32> = triangle.p0 * barycentrics.x + triangle.p1 * barycentrics.y + triangle.p2 * barycentrics.z;

    //             bsdf_light_sample = LightSample::new_triangle_sample(point, material.emission, triangle);

    //             let sample_emission: vec3<f32> = LightSample::intensity(bsdf_light_sample, hit_point_ws) * material.emission; // TODO: move down
    //             bsdf_phat = linear_to_luma(contribution * sample_emission);

    //             return DiReservoir(1.0, 1.0 / bsdf_sample_pdf, 0.0, 0.0, bsdf_light_sample);
    //         }
    //     }
    // }

    // return DiReservoir(0.0, 0.0, 0.0, 0.0, LightSample::empty());

    // var sample_pdf: f32;
    // let sample: LightSample = Nee::sample_uniform(random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
    //     vec2<f32>(random_uniform_float(rng), random_uniform_float(rng)), hit_point_ws, &sample_pdf);
    // return DiReservoir(1.0, 1.0 / sample_pdf, 0.0, 0.0, sample);

    // const NUM_SAMPLES: u32 = 32;

    // var di_reservoir = DiReservoir::new();

    // for (var i: u32 = 0; i < NUM_SAMPLES; i += 1) {
    //     var sample_pdf: f32;
    //     let sample: LightSample = Nee::sample_uniform(random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
    //         vec2<f32>(random_uniform_float(rng), random_uniform_float(rng)), hit_point_ws, &sample_pdf);

    //     let w_in_worldspace: vec3<f32> = normalize(sample.point - hit_point_ws);
    //     let n_dot_l: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);

    //     var phat: f32 = 0.0;
    //     var weight: f32 = 0.0;
    //     if (n_dot_l > 0.0 && sample_pdf > 0.0) {
    //         var shading_pdf: f32;
    //         let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
    //             tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
    //             w_out_worldspace, w_in_worldspace, &shading_pdf);
            
    //         if (shading_pdf > 0.0) {
    //             let contribution: vec3<f32> = n_dot_l * reflectance;
    //             let sample_intensity: f32 = LightSample::intensity(sample, hit_point_ws);
    //             phat = linear_to_luma(contribution * sample_intensity);

    //             // 𝑤_𝑖 ← 𝑚_𝑖(𝑋_𝑖) 𝑝ˆ(𝑋_𝑖) 𝑊_𝑋_𝑖
    //             weight = (1.0 / f32(NUM_SAMPLES)) * phat * (1.0 / sample_pdf);
    //         }
    //     }

    //     DiReservoir::update(&di_reservoir, weight, rng, sample, phat);
    // }

    const NUM_AREA_SAMPLES: u32 = 4;
    const NUM_BSDF_SAMPLES: u32 = 1;

    var di_reservoir = DiReservoir::new();
    
    for (var i: u32 = 0; i < max(NUM_AREA_SAMPLES, NUM_BSDF_SAMPLES); i += 1) {
        var area_sample_pdf: f32 = 0.0;
        var area_light_sample: LightSample;
        var area_phat: f32 = 0.0;
        if (i < NUM_AREA_SAMPLES) {
            area_light_sample = Nee::sample_uniform(random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
                vec2<f32>(random_uniform_float(rng), random_uniform_float(rng)), hit_point_ws, &area_sample_pdf);

            let w_in_worldspace: vec3<f32> = normalize(area_light_sample.point - hit_point_ws);
            let wi_dot_n: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);

            if (wi_dot_n > 0.0 && area_sample_pdf > 0.0) {
                var shading_pdf: f32;
                let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
                    tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                    w_out_worldspace, w_in_worldspace, &shading_pdf);
                
                if (shading_pdf > 0.0) {
                    let contribution: vec3<f32> = wi_dot_n * reflectance;
                    let sample_emission: vec3<f32> = LightSample::intensity(area_light_sample, hit_point_ws) * area_light_sample.emission;
                    area_phat = linear_to_luma(contribution * sample_emission);
                }
            }
        }

        var bsdf_sample_pdf: f32 = 0.0;
        var bsdf_light_sample = LightSample::empty();
        var bsdf_phat: f32 = 0.0;
        if (i < NUM_BSDF_SAMPLES) {
            var w_in_worldspace: vec3<f32>;
            var specular: bool;
            let reflectance: vec3<f32> = DisneyBsdf::sample(disney_bsdf,
                front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                w_out_worldspace, t, back_face,
                random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
                &w_in_worldspace, &bsdf_sample_pdf, &specular
            );

            let wi_dot_n: f32 = abs(dot(w_in_worldspace, front_facing_shading_normal_ws));
            let contribution: vec3<f32> = wi_dot_n * reflectance;

            if (dot(contribution, contribution) > 0.0) {
                // TODO: non-opaques
                var rq: ray_query;
                rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, 1000.0, safe_origin(hit_point_ws, front_facing_shading_normal_ws), w_in_worldspace));
                rayQueryProceed(&rq);
                let intersection = rayQueryGetCommittedIntersection(&rq);
                if (intersection.kind == RAY_QUERY_INTERSECTION_TRIANGLE) {
                    let vertex_pool_slice_index: u32 = intersection.instance_custom_data;
                    let vertex_pool_slice: VertexPoolSlice = vertex_pool_slices[vertex_pool_slice_index];

                    let barycentrics = vec3<f32>(1.0 - intersection.barycentrics.x - intersection.barycentrics.y, intersection.barycentrics);

                    let i0: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 0];
                    let i1: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 1];
                    let i2: u32 = vertex_indices[vertex_pool_slice.first_index + intersection.primitive_index * 3 + 2];

                    let v0: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i0]);
                    let v1: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i1]);
                    let v2: Vertex = PackedVertex::unpack(vertices[vertex_pool_slice.first_vertex + i2]);

                    let tex_coord: vec2<f32> = v0.tex_coord * barycentrics.x + v1.tex_coord * barycentrics.y + v2.tex_coord * barycentrics.z;

                    let material_idx: u32 = vertex_pool_slice.material_idx + triangle_material_indices[vertex_pool_slice.first_index / 3 + intersection.primitive_index];
                    let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
                    let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord);
                    if (dot(material.emission, material.emission) > 0.0 || true) {
                        var triangle = Triangle::new(
                            (intersection.object_to_world * vec4<f32>(v0.position, 1.0)).xyz,
                            (intersection.object_to_world * vec4<f32>(v1.position, 1.0)).xyz,
                            (intersection.object_to_world * vec4<f32>(v2.position, 1.0)).xyz
                        );
                        let point: vec3<f32> = hit_point_ws + w_in_worldspace * intersection.t;

                        bsdf_light_sample = LightSample::new_triangle_sample(point, material.emission, triangle);
                    }
                } else {
                    bsdf_light_sample = LightSample::new_sun_sample(w_in_worldspace * SUN_DISTANCE, sky_constants.sun_color);
                }

                if (!LightSample::is_empty(bsdf_light_sample)) {
                    let sample_emission: vec3<f32> = LightSample::intensity(bsdf_light_sample, hit_point_ws) * bsdf_light_sample.emission;
                    bsdf_phat = linear_to_luma(contribution * sample_emission);
                }
            }
        }
        
        if (i < NUM_AREA_SAMPLES) {
            var area_weight: f32 = 0.0;
            if (area_sample_pdf > 0.0) {
                let mis_weight: f32 = balance_heuristic(area_sample_pdf, f32(NUM_AREA_SAMPLES), bsdf_sample_pdf, f32(NUM_BSDF_SAMPLES));
                // 𝑤_𝑖 ← 𝑚_𝑖(𝑋_𝑖) 𝑝ˆ(𝑋_𝑖) 𝑊_𝑋_𝑖
                area_weight = mis_weight * area_phat * (1.0 / max(area_sample_pdf, 1e-8));
            }

            DiReservoir::update(&di_reservoir, area_weight, rng, area_light_sample, area_phat);
        }

        if (i < NUM_BSDF_SAMPLES) {
            var bsdf_weight: f32 = 0.0;
            if (bsdf_sample_pdf > 0.0) {
                let mis_weight: f32 = balance_heuristic(bsdf_sample_pdf, f32(NUM_BSDF_SAMPLES), area_sample_pdf, f32(NUM_AREA_SAMPLES));
                // 𝑤_𝑖 ← 𝑚_𝑖(𝑋_𝑖) 𝑝ˆ(𝑋_𝑖) 𝑊_𝑋_𝑖
                bsdf_weight = mis_weight * bsdf_phat * (1.0 / max(bsdf_sample_pdf, 1e-8));
            }

            DiReservoir::update(&di_reservoir, bsdf_weight, rng, bsdf_light_sample, bsdf_phat);
        }
    }

    if (di_reservoir.selected_phat > RESTIR_DI_EPSILON) {
        let direction: vec3<f32> = normalize(di_reservoir.sample.point - hit_point_ws);
        let distance: f32 = distance(di_reservoir.sample.point, hit_point_ws);

        if (trace_shadow_ray(hit_point_ws, direction, distance, front_facing_shading_normal_ws, scene)) {
            // 𝑟.𝑊_𝑌 ← (1 / 𝑝ˆ(𝑟.𝑌)) 𝑟.𝑤_sum
            di_reservoir.contribution_weight = (1.0 / di_reservoir.selected_phat) * di_reservoir.weight_sum;
        }
    }

    return di_reservoir;
}