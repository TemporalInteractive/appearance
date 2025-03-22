@include ::color
@include appearance-path-tracer-gpu::shared/ray
@include appearance-path-tracer-gpu::shared/restir/gi_reservoir
@include appearance-path-tracer-gpu::shared/material/disney_bsdf

@include appearance-path-tracer-gpu::helpers/trace

///
/// BINDING DEPENDENCIES:
/// appearance-path-tracer-gpu::shared/vertex_pool_bindings
/// appearance-path-tracer-gpu::shared/material/material_pool_bindings
/// appearance-path-tracer-gpu::shared/sky_bindings
///

/// Returns radiance traced along the path starting at origin
fn InlinePathTracer::trace(_origin: vec3<f32>, _direction: vec3<f32>, max_bounces: u32, throughput: ptr<function, vec3<f32>>, first_hit_ws: ptr<function, vec3<f32>>, rng: ptr<function, u32>, scene: acceleration_structure) -> vec3<f32> {
    var origin: vec3<f32> = _origin;
    var direction: vec3<f32> = _direction;

    var accumulated = vec3<f32>(0.0);

    for (var bounce: u32 = 0; bounce < max_bounces; bounce += 1) {
        var safe_origin_normal: vec3<f32> = direction;
        for (var step: u32 = 0; step < MAX_NON_OPAQUE_DEPTH; step += 1) {
            if (dot(safe_origin_normal, direction) < 0.0) {
                safe_origin_normal *= -1.0;
            }

            var rq: ray_query;
            rayQueryInitialize(&rq, scene, RayDesc(0u, 0xFFu, 0.0, safe_distance(1000.0), safe_origin(origin, safe_origin_normal), direction));
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
                var material_color: vec4<f32> = MaterialDescriptor::color(material_descriptor, tex_coord);

                if (material_color.a < material_descriptor.alpha_cutoff) {
                    if (step + 1 < MAX_NON_OPAQUE_DEPTH) {
                        var p01: vec3<f32> = v1.position - v0.position;
                        var p02: vec3<f32> = v2.position - v0.position;
                        safe_origin_normal = normalize(cross(p01, p02));
    
                        // TODO: non-opaque geometry would be a better choice, not properly supported by wgpu yet
                        origin += direction * intersection.t;
                        continue;
                    } else {
                        material_color.a = 1.0;
                    }
                }
                let material: Material = Material::from_material_descriptor_with_color(material_descriptor, tex_coord, material_color);

                // Load tangent, bitangent and normal in local space
                let tbn: mat3x3<f32> = VertexPoolBindings::load_tbn(v0, v1, v2, barycentrics);

                // Calculate local to world matrix, inversed and transposed
                let local_to_world_inv = mat4x4<f32>(
                    vec4<f32>(intersection.world_to_object[0], 0.0),
                    vec4<f32>(intersection.world_to_object[1], 0.0),
                    vec4<f32>(intersection.world_to_object[2], 0.0),
                    vec4<f32>(0.0, 0.0, 0.0, 1.0)
                );
                let local_to_world_inv_trans: mat4x4<f32> = transpose(local_to_world_inv);

                // World space tangent, bitangent and normal. Note that these are not front facing yet
                let hit_tangent_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[0], 1.0)).xyz);
                let hit_bitangent_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[1], 1.0)).xyz);
                var hit_normal_ws: vec3<f32> = normalize((local_to_world_inv_trans * vec4<f32>(tbn[2], 1.0)).xyz);
                let hit_point_ws = origin + direction * intersection.t;

                let hit_tangent_to_world = mat3x3<f32>(
                    hit_tangent_ws,
                    hit_bitangent_ws,
                    hit_normal_ws
                );

                // Apply normal mapping when available, unlike the name suggest, still not front facing
                var front_facing_normal_ws: vec3<f32> = hit_normal_ws;
                var front_facing_shading_normal_ws: vec3<f32> = MaterialDescriptor::apply_normal_mapping(material_descriptor, tex_coord, hit_normal_ws, hit_tangent_to_world);

                let w_out_worldspace: vec3<f32> = -direction;

                // Make sure the hit normal and normal mapped normal are front facing
                let back_face: bool = dot(w_out_worldspace, hit_normal_ws) < 0.0;
                if (back_face) {
                    front_facing_normal_ws *= -1.0;
                    front_facing_shading_normal_ws *= -1.0;
                }

                // Construct tangent <-> world matrices
                let tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_shading_normal_ws);
                let world_to_tangent: mat3x3<f32> = transpose(tangent_to_world);

                var clearcoat_tangent_to_world: mat3x3<f32>;
                var clearcoat_world_to_tangent: mat3x3<f32>;
                if (material_descriptor.clearcoat > 0.0) {
                    if (material_descriptor.clearcoat_normal_texture == INVALID_TEXTURE) {
                        clearcoat_tangent_to_world = build_orthonormal_basis(front_facing_normal_ws);
                        clearcoat_world_to_tangent = transpose(clearcoat_tangent_to_world);
                    } else if (material_descriptor.clearcoat_normal_texture == material_descriptor.normal_texture) {
                        clearcoat_tangent_to_world = tangent_to_world;
                        clearcoat_world_to_tangent = world_to_tangent;
                    } else {
                        var front_facing_clearcoat_normal_ws: vec3<f32> = MaterialDescriptor::apply_clearcoat_normal_mapping(material_descriptor, tex_coord, hit_normal_ws, hit_tangent_to_world);
                        if (back_face) {
                            front_facing_clearcoat_normal_ws *= -1.0;
                        }
                        clearcoat_tangent_to_world = build_orthonormal_basis(front_facing_clearcoat_normal_ws);
                        clearcoat_world_to_tangent = transpose(clearcoat_tangent_to_world);
                    }
                }

                let disney_bsdf = DisneyBsdf::from_material(material);

                let di_reservoir: DiReservoir = Nee::sample_ris(hit_point_ws, w_out_worldspace, front_facing_shading_normal_ws,
                    tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                    disney_bsdf, intersection.t, back_face, rng, scene);
                let light_sample: LightSample = di_reservoir.sample;
                if (di_reservoir.contribution_weight > 0.0) {
                    let shadow_direction: vec3<f32> = normalize(light_sample.point - hit_point_ws);
                    let shadow_distance: f32 = distance(light_sample.point, hit_point_ws);
                    let n_dot_l: f32 = dot(shadow_direction, front_facing_shading_normal_ws);

                    if (n_dot_l > 0.0) {
                        if (trace_shadow_ray(hit_point_ws, shadow_direction, shadow_distance, front_facing_shading_normal_ws, scene)) {
                            let w_in_worldspace: vec3<f32> = shadow_direction;

                            var shading_pdf: f32;
                            let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                                w_out_worldspace, w_in_worldspace, &shading_pdf);

                            let light_intensity: vec3<f32> = LightSample::intensity(light_sample, hit_point_ws) * light_sample.emission;

                            let contribution: vec3<f32> = (*throughput) * reflectance * light_intensity * n_dot_l * di_reservoir.contribution_weight;
                            accumulated += contribution;
                        };
                    }
                }

                if (bounce == 0) {
                    *first_hit_ws = hit_point_ws;
                }
                
                if (bounce + 1 < max_bounces) {
                    if (bounce > 1) {
                        let russian_roulette: f32 = max((*throughput).r, max((*throughput).g, (*throughput).b));

                        if (russian_roulette < random_uniform_float(rng)) {
                            return accumulated;
                        } else {
                            (*throughput) *= 1.0 / russian_roulette;
                        }
                    }

                    var w_in_worldspace: vec3<f32>;
                    var pdf: f32;
                    var specular: bool;
                    let reflectance: vec3<f32> = DisneyBsdf::sample(disney_bsdf,
                        front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
                        w_out_worldspace, intersection.t, back_face,
                        random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
                        &w_in_worldspace, &pdf, &specular
                    );

                    let sample_valid: bool = pdf > 1e-6;
                    if (sample_valid) {
                        let cos_in: f32 = abs(dot(front_facing_shading_normal_ws, w_in_worldspace));
                        let contribution: vec3<f32> = (1.0 / pdf) * reflectance * cos_in;
                        (*throughput) *= contribution;

                        origin = hit_point_ws;
                        direction = w_in_worldspace;
                    } else {
                        return accumulated;
                    }
                }
            } else {
                if (bounce == 0) {
                    *first_hit_ws = origin + direction * 1000.0;
                }

                let color = Sky::sky(direction, true);
                accumulated += (*throughput) * color;
                return accumulated;
            }

            break;
        }
    }

    return accumulated;
}

fn InlinePathTracer::sample_ris(hit_point_ws: vec3<f32>, w_out_worldspace: vec3<f32>, front_facing_shading_normal_ws: vec3<f32>,
     tangent_to_world: mat3x3<f32>, world_to_tangent: mat3x3<f32>, clearcoat_tangent_to_world: mat3x3<f32>, clearcoat_world_to_tangent: mat3x3<f32>,
     disney_bsdf: DisneyBsdf, throughput: vec3<f32>, t: f32, back_face: bool, rng: ptr<function, u32>, scene: acceleration_structure) -> GiReservoir {
    const NUM_SAMPLES: u32 = 1;

    var gi_reservoir = GiReservoir::new();

    for (var i: u32 = 0; i < NUM_SAMPLES; i += 1) {
        var w_in_worldspace: vec3<f32>;
        var pdf: f32;
        var specular: bool;
        let reflectance: vec3<f32> = DisneyBsdf::sample(disney_bsdf,
            front_facing_shading_normal_ws, tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
            w_out_worldspace, t, back_face,
            random_uniform_float(rng), random_uniform_float(rng), random_uniform_float(rng),
            &w_in_worldspace, &pdf, &specular
        );
        
        if (pdf > 1e-6) {
            let cos_in: f32 = abs(dot(w_in_worldspace, front_facing_shading_normal_ws));
            let local_throughput: vec3<f32> = cos_in * reflectance;

            let gi_origin: vec3<f32> = hit_point_ws + w_in_worldspace * 0.0001;
            let gi_direction: vec3<f32> = w_in_worldspace;

            var throughput_result: vec3<f32> = throughput * local_throughput;
            let phat_rng: u32 = *rng;
            var sample_point_ws: vec3<f32>;
            let contribution: vec3<f32> = InlinePathTracer::trace(gi_origin, gi_direction, RESTIR_GI_PHAT_MAX_BOUNCES, &throughput_result, &sample_point_ws, rng, scene);

            let phat: f32 = linear_to_luma(contribution);
            let weight: f32 = phat / pdf;
            GiReservoir::update(&gi_reservoir, weight, rng, sample_point_ws, phat, phat_rng);
        }
    }

    if (gi_reservoir.selected_phat > 0.0 && gi_reservoir.sample_count * gi_reservoir.weight_sum > 0.0) {
        gi_reservoir.contribution_weight = (1.0 / gi_reservoir.selected_phat) * (1.0 / gi_reservoir.sample_count * gi_reservoir.weight_sum);
    }

    return gi_reservoir;
}

// TODO: maybe use throughput is lightsample ctx?
fn GiReservoir::phat(_self: GiReservoir, light_sample_ctx: LightSampleCtx, throughput: vec3<f32>, hit_point_ws: vec3<f32>, w_out_worldspace: vec3<f32>, scene: acceleration_structure) -> f32 {
    let tex_coord: vec2<f32> = light_sample_ctx.hit_tex_coord;
    let material_idx: u32 = light_sample_ctx.hit_material_idx;
    let material_descriptor: MaterialDescriptor = material_descriptors[material_idx];
    let material: Material = Material::from_material_descriptor(material_descriptor, tex_coord);
    let disney_bsdf = DisneyBsdf::from_material(material);

    let front_facing_shading_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(light_sample_ctx.front_facing_shading_normal_ws, 0);
    let tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_shading_normal_ws);
    let world_to_tangent: mat3x3<f32> = transpose(tangent_to_world);

    let front_facing_clearcoat_normal_ws: vec3<f32> = PackedNormalizedXyz10::unpack(light_sample_ctx.front_facing_clearcoat_normal_ws, 0);
    let clearcoat_tangent_to_world: mat3x3<f32> = build_orthonormal_basis(front_facing_clearcoat_normal_ws);
    let clearcoat_world_to_tangent: mat3x3<f32> = transpose(clearcoat_tangent_to_world);
    
    let w_in_worldspace: vec3<f32> = normalize(_self.sample_point_ws - hit_point_ws);

    var shading_pdf: f32;
    let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
        tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
        w_out_worldspace, w_in_worldspace, &shading_pdf);
    var cos_in: f32 = abs(dot(w_in_worldspace, front_facing_shading_normal_ws));
    //jacobianDiffuse(current_gbuffer_texel.position_ws, prev_gbuffer_texel.position_ws, prev_normal_ws, w_in_worldspace, payload.t);

    let local_throughput: vec3<f32> = cos_in * reflectance;
    let gi_origin: vec3<f32> = hit_point_ws + w_in_worldspace * 0.0001;
    let gi_direction: vec3<f32> = w_in_worldspace;
    var throughput_result: vec3<f32> = throughput * local_throughput;
    var phat_rng: u32 = _self.phat_rng;
    var sample_point_ws: vec3<f32>;
    let contribution: vec3<f32> = InlinePathTracer::trace(gi_origin, gi_direction, RESTIR_GI_PHAT_MAX_BOUNCES, &throughput_result, &sample_point_ws, &phat_rng, scene);
    return linear_to_luma(contribution);
}

fn GiReservoir::combine_unbiased(r1: GiReservoir, r1_hit_point_ws: vec3<f32>, r1_light_sample_ctx: LightSampleCtx, r1_w_out_worldspace: vec3<f32>, r1_throughput: vec3<f32>,
                                  r2: GiReservoir, r2_hit_point_ws: vec3<f32>, r2_light_sample_ctx: LightSampleCtx, r2_w_out_worldspace: vec3<f32>, r2_throughput: vec3<f32>,
                                  rng: ptr<function, u32>, scene: acceleration_structure) -> GiReservoir {
    var combined_reservoir = GiReservoir::new();
    GiReservoir::update(&combined_reservoir, r1.selected_phat * r1.contribution_weight * r1.sample_count, rng, r1.sample_point_ws, r1.selected_phat, r1.phat_rng);
    GiReservoir::update(&combined_reservoir, r2.selected_phat * r2.contribution_weight * r2.sample_count, rng, r2.sample_point_ws, r2.selected_phat, r2.phat_rng);
    combined_reservoir.sample_count = r1.sample_count + r2.sample_count;

    var z: f32 = 0.0;
    if (GiReservoir::phat(combined_reservoir, r1_light_sample_ctx, r1_throughput, r1_hit_point_ws, r1_w_out_worldspace, scene) > 0.0) {
    //if (LightSample::phat(combined_reservoir.sample, r1_light_sample_ctx, r1_hit_point_ws, r1_w_out_worldspace, true, scene) > 0.0) {
        z += r1.sample_count;
    }
    if (GiReservoir::phat(combined_reservoir, r2_light_sample_ctx, r2_throughput, r2_hit_point_ws, r2_w_out_worldspace, scene) > 0.0) {
    //if (LightSample::phat(combined_reservoir.sample, r2_light_sample_ctx, r2_hit_point_ws, r2_w_out_worldspace, true, scene) > 0.0) {
        z += r2.sample_count;
    }

    if (combined_reservoir.selected_phat > 0.0 && z * combined_reservoir.weight_sum > 0.0) {
        combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / z * combined_reservoir.weight_sum);
    }

    return combined_reservoir;
}