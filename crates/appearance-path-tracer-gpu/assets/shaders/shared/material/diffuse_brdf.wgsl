@include appearance-path-tracer-gpu::shared/material/bsdf

struct DiffuseLobe {
    albedo: vec3<f32>,
}

fn DiffuseLobe::new(albedo: vec3<f32>) -> DiffuseLobe {
    return DiffuseLobe(albedo);
}

fn DiffuseLobe::sample(lobe: DiffuseLobe, uv: vec2<f32>) -> BsdfSample {
    var sample: BsdfSample;
    sample.refract = false;
    sample.w_in_tangent_space = get_cosine_hemisphere_sample(uv);
    sample.pdf = diffuse_brdf_pdf(sample.w_in_tangent_space.z);
    return sample;
}

fn DiffuseLobe::eval(lobe: DiffuseLobe) -> BsdfEval {
    var eval: BsdfEval;
    eval.reflectance = diffuse_brdf_eval(lobe.albedo);
    eval.fraction_transmitted = vec3<f32>(0.0);
    return eval;
}