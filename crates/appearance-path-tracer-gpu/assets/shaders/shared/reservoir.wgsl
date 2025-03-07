@include ::random

struct Reservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    _padding: u32,
}

fn Reservoir::new() -> Reservoir {
    return Reservoir(0.0, 0.0, 0.0, 0);
}

fn Reservoir::update(_self: ptr<function, Reservoir>, sample_weight: f32, rng: ptr<function, u32>) -> bool {
    (*_self).weight_sum += sample_weight;
    (*_self).sample_count += 1.0;
    
    return random_uniform_float(rng) <= (sample_weight / (*_self).weight_sum);
}