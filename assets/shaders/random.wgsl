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

// Combine hash, taken from Kajiya
fn hash_combine(x: u32, y: u32) -> u32 {
    const M: u32 = 1664525;
    const C: u32 = 1013904223u;
    var seed: u32 = (x * M + y + C) * M;

    // Tempering (from Matsumoto)
    seed ^= (seed >> 11u);
    seed ^= (seed << 7u) & 0x9d2c5680u;
    seed ^= (seed << 15u) & 0xefc60000u;
    seed ^= (seed >> 18u);
    return seed;
}