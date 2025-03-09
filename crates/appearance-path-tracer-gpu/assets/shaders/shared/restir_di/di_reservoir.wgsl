@include ::random
@include appearance-path-tracer-gpu::shared/light_sample

struct DiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    sample: LightSample,
}

struct PackedDiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    _padding0: u32,
    sample: PackedLightSample,
}

fn DiReservoir::new() -> DiReservoir {
    return DiReservoir(0.0, 0.0, 0.0, LightSample::empty());
}

fn PackedDiReservoir::new(di_reservoir: DiReservoir) -> PackedDiReservoir {
    return PackedDiReservoir(
        di_reservoir.sample_count,
        di_reservoir.contribution_weight,
        di_reservoir.weight_sum,
        0,
        PackedLightSample::new(di_reservoir.sample)
    );
}

fn PackedDiReservoir::unpack(_self: PackedDiReservoir) -> DiReservoir {
    return DiReservoir(
        _self.sample_count,
        _self.contribution_weight,
        _self.weight_sum,
        PackedLightSample::unpack(_self.sample)
    );
}

fn DiReservoir::update(_self: ptr<function, DiReservoir>, sample_weight: f32, rng: ptr<function, u32>, sample: LightSample) -> bool {
    (*_self).weight_sum += sample_weight;
    (*_self).sample_count += 1.0;

    if (random_uniform_float(rng) <= (sample_weight / (*_self).weight_sum)) {
        (*_self).sample = sample;
        return true;
    }
    return false;
}