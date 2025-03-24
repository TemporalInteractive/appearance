@include ::random
@include appearance-path-tracer-gpu::shared/light_sample

const RESTIR_DI_EPSILON: f32 = 1e-6;

struct DiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    sample: LightSample,
}

struct PackedDiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    sample: PackedLightSample,
}

fn DiReservoir::new() -> DiReservoir {
    return DiReservoir(0.0, 0.0, 0.0, 0.0, LightSample::empty());
}

fn PackedDiReservoir::new(di_reservoir: DiReservoir) -> PackedDiReservoir {
    return PackedDiReservoir(
        di_reservoir.sample_count,
        di_reservoir.contribution_weight,
        di_reservoir.weight_sum,
        di_reservoir.selected_phat,
        PackedLightSample::new(di_reservoir.sample)
    );
}

fn PackedDiReservoir::unpack(_self: PackedDiReservoir) -> DiReservoir {
    return DiReservoir(
        _self.sample_count,
        _self.contribution_weight,
        _self.weight_sum,
        _self.selected_phat,
        PackedLightSample::unpack(_self.sample)
    );
}

fn DiReservoir::update(_self: ptr<function, DiReservoir>, sample_weight: f32, rng: ptr<function, u32>, sample: LightSample, phat: f32) -> bool {
    (*_self).weight_sum += sample_weight;
    (*_self).sample_count += 1.0;

    if (random_uniform_float(rng) <= (sample_weight / (*_self).weight_sum)) {
        (*_self).sample = sample;
        (*_self).selected_phat = phat;
        return true;
    }
    return false;
}

fn LightSample::phat(_self: LightSample, light_sample_ctx: LightSampleCtx, hit_point_ws: vec3<f32>, w_out_worldspace: vec3<f32>, visibility_test: bool, scene: acceleration_structure) -> f32 {
    let light_sample_eval_data: LightSampleEvalData = LightSample::load_eval_data(_self, hit_point_ws);

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

    let w_in_worldspace: vec3<f32> = normalize(light_sample_eval_data.point_ws - hit_point_ws);

    var visibility: bool = true;
    if (visibility_test) {
        let distance: f32 = distance(light_sample_eval_data.point_ws, hit_point_ws);
        visibility = trace_shadow_ray(hit_point_ws, w_in_worldspace, distance, front_facing_shading_normal_ws, scene);
    }

    let wi_dot_n: f32 = dot(w_in_worldspace, front_facing_shading_normal_ws);
    if (wi_dot_n > 0.0 && visibility) {
        var shading_pdf: f32;
        let reflectance: vec3<f32> = DisneyBsdf::evaluate(disney_bsdf, front_facing_shading_normal_ws,
            tangent_to_world, world_to_tangent, clearcoat_tangent_to_world, clearcoat_world_to_tangent,
            w_out_worldspace, w_in_worldspace, &shading_pdf);

        // 𝑝ˆ(𝑥) = 𝑓_𝑠(𝑥) 𝐺(𝑥) 𝑉(𝑥) 𝐿_𝑒(𝑥)
        return linear_to_luma(reflectance * wi_dot_n * light_sample_eval_data.emission);
    }

    return 0.0;
}

fn DiReservoir::combine(r1: DiReservoir, r2: DiReservoir, rng: ptr<function, u32>) -> DiReservoir {
    var combined_reservoir = DiReservoir::new();
    DiReservoir::update(&combined_reservoir, r1.selected_phat * r1.contribution_weight * r1.sample_count, rng, r1.sample, r1.selected_phat);
    DiReservoir::update(&combined_reservoir, r2.selected_phat * r2.contribution_weight * r2.sample_count, rng, r2.sample, r2.selected_phat);
    combined_reservoir.sample_count = r1.sample_count + r2.sample_count;

    if (combined_reservoir.selected_phat > RESTIR_DI_EPSILON && combined_reservoir.sample_count * combined_reservoir.weight_sum > RESTIR_DI_EPSILON) {
        combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / combined_reservoir.sample_count * combined_reservoir.weight_sum);
    }

    return combined_reservoir;
}

fn DiReservoir::combine_unbiased(r1: DiReservoir, r1_hit_point_ws: vec3<f32>, r1_light_sample_ctx: LightSampleCtx, r1_w_out_worldspace: vec3<f32>,
                                  r2: DiReservoir, r2_hit_point_ws: vec3<f32>, r2_light_sample_ctx: LightSampleCtx, r2_w_out_worldspace: vec3<f32>,
                                  rng: ptr<function, u32>, scene: acceleration_structure) -> DiReservoir {
    var combined_reservoir = DiReservoir::new();
    DiReservoir::update(&combined_reservoir, r1.selected_phat * r1.contribution_weight * r1.sample_count, rng, r1.sample, r1.selected_phat);
    DiReservoir::update(&combined_reservoir, r2.selected_phat * r2.contribution_weight * r2.sample_count, rng, r2.sample, r2.selected_phat);
    combined_reservoir.sample_count = r1.sample_count + r2.sample_count;

    var z: f32 = 0.0;
    if (LightSample::phat(combined_reservoir.sample, r1_light_sample_ctx, r1_hit_point_ws, r1_w_out_worldspace, true, scene) > 0.0) {
        z += r1.sample_count;
    }
    if (LightSample::phat(combined_reservoir.sample, r2_light_sample_ctx, r2_hit_point_ws, r2_w_out_worldspace, true, scene) > 0.0) {
        z += r2.sample_count;
    }

    if (combined_reservoir.selected_phat > RESTIR_DI_EPSILON && z * combined_reservoir.weight_sum > RESTIR_DI_EPSILON) {
        combined_reservoir.contribution_weight = (1.0 / combined_reservoir.selected_phat) * (1.0 / z * combined_reservoir.weight_sum);
    }

    return combined_reservoir;
}