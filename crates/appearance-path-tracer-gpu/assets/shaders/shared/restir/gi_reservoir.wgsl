@include ::random
@include appearance-packing::shared/packing

const RESTIR_GI_PHAT_MAX_BOUNCES: u32 = 1;

struct GiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    w_in_worldspace: vec3<f32>,
}

struct PackedGiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    w_in_worldspace: PackedNormalizedXyz10,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

fn GiReservoir::new() -> GiReservoir {
    return GiReservoir(0.0, 0.0, 0.0, 0.0, vec3<f32>(0.0));
}

fn PackedGiReservoir::new(gi_reservoir: GiReservoir) -> PackedGiReservoir {
    return PackedGiReservoir(
        gi_reservoir.sample_count,
        gi_reservoir.contribution_weight,
        gi_reservoir.weight_sum,
        gi_reservoir.selected_phat,
        PackedNormalizedXyz10::new(gi_reservoir.w_in_worldspace, 0),
        0,
        0,
        0
    );
}

fn PackedGiReservoir::unpack(_self: PackedGiReservoir) -> GiReservoir {
    return GiReservoir(
        _self.sample_count,
        _self.contribution_weight,
        _self.weight_sum,
        _self.selected_phat,
        PackedNormalizedXyz10::unpack(_self.w_in_worldspace, 0)
    );
}

fn GiReservoir::update(_self: ptr<function, GiReservoir>, sample_weight: f32, rng: ptr<function, u32>, w_in_worldspace: vec3<f32>, phat: f32) -> bool {
    (*_self).weight_sum += sample_weight;
    (*_self).sample_count += 1.0;

    if (random_uniform_float(rng) <= (sample_weight / (*_self).weight_sum)) {
        (*_self).w_in_worldspace = w_in_worldspace;
        (*_self).selected_phat = phat;
        return true;
    }
    return false;
}
