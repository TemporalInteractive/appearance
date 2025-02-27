fn wangh_hash(x: u32) -> u32 {
    var state: u32 = (x ^ 61u) ^ (x >> 16);
    state *= 9u;
    state = state ^ (state >> 4);
    state *= 0x27d4eb2du;
    state = state ^ (state >> 15);
    return state;
}

fn pcg_hash(x: u32) -> u32 {
    var state: u32 = x * 747796405u + 2891336453u;
    var word: u32 = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random_uniform_float(state: ptr<function, u32>) -> f32 {
    *state = pcg_hash(*state);
    return f32(*state) / f32(0xFFFFFFFF);
}

fn random_uniform_float2(state: ptr<function, u32>) -> vec2<f32> {
    return vec2<f32>(
        random_uniform_float(state),
        random_uniform_float(state)
    );
}

fn random_uniform_float3(state: ptr<function, u32>) -> vec3<f32> {
    return vec3<f32>(
        random_uniform_float(state),
        random_uniform_float(state),
        random_uniform_float(state)
    );
}

fn random_uniform_float4(state: ptr<function, u32>) -> vec4<f32> {
    return vec4<f32>(
        random_uniform_float(state),
        random_uniform_float(state),
        random_uniform_float(state),
        random_uniform_float(state)
    );
}

fn random_uniform_float_ranged(state: ptr<function, u32>, lo: f32, hi: f32) -> f32 {
    return lo + (random_uniform_float(state) * (hi - lo));
}

fn xor_shift_u32(state: u32) -> u32 {
    var s: u32 = state ^ (state << 13);
    s ^= s >> 17;
    s ^= s << 5;
    return s;
}