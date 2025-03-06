@include ::math
@include ::tangent_space_trig_helpers
@include appearance-path-tracer-gpu::shared/sampling

const MIN_ROUGHNESS_THRESHOLD: f32 = 0.08f;
const MIN_COS_THETA_THRESHOLD: f32 = 1e-6f;
const MIN_BRDF_PDF_THRESHOLD: f32 = 1e-9f;
const METALLIC_THRESHOLD: f32 = 1e-3f;
const SQUARED_ABSORPTION_THRESHOLD: f32 = 1e-6f;

const IOR_AIR: f32 = 1.000293f;

const _FE: array<u32, 16> = array<u32, 16>(
    1195132717, 3295168244, 3319579794, 3112876607,
    1127038013, 1100497709, 3364538495, 1155975142,
    1147356411, 939276304, 3051632581, 3051009512,
    781201040, 11983, 0, 0,
);

struct BsdfSample {
    w_in_tangent_space: vec3<f32>,
    pdf: f32,
    refract: bool,
}

struct BsdfEval {
    reflectance: vec3<f32>,
    fraction_transmitted: vec3<f32>,
}

fn apply_bsdf(
    sample: BsdfSample,
    eval: BsdfEval,
    tangent_to_world: mat3x3<f32>,
    normal: vec3<f32>,
    throughput: ptr<function, vec3<f32>>,
    w_in_worldspace: ptr<function, vec3<f32>>
) -> bool {
    if (!bsdf_sample_valid(sample)) {
        return false;
    }

    *w_in_worldspace = normalize(tangent_to_world * sample.w_in_tangent_space);
    let cos_in: f32 = abs(dot(normal, *w_in_worldspace));

    var contribution: vec3<f32> = (1.0 / sample.pdf) * eval.reflectance * cos_in;
    *throughput = *throughput * contribution;
    
    return true;
}

fn bsdf_sample_valid(sample: BsdfSample) -> bool {
    return (sample.w_in_tangent_space.z > MIN_COS_THETA_THRESHOLD || sample.refract) && sample.pdf > MIN_BRDF_PDF_THRESHOLD;
}

fn diffuse_brdf_pdf(cos_theta: f32) -> f32 {
    return max(0.0, cos_theta) * INV_PI;
}

fn diffuse_brdf_eval(base_color: vec3<f32>) -> vec3<f32> {
    return INV_PI * base_color;
}

fn refract_dir(w_in_tangent_space: vec3<f32>, relative_ior: f32) -> vec3<f32> {
    return -refract(-w_in_tangent_space, vec3<f32>(0.0, 0.0, 1.0), 1.0 / relative_ior);
}

fn calculate_absorption(layer_color: vec3<f32>, thickness: f32, cos_theta_wo: f32, cos_theta_wi: f32) -> vec3<f32> {
    let inverse_color: vec3<f32> = 1.0 - layer_color;
    let inverse_cos_theta_i: f32 = 1.0 / min(1.0, max(MIN_COS_THETA_THRESHOLD, cos_theta_wi));
    let inverse_cos_theta_o: f32 = 1.0 / min(1.0, max(MIN_COS_THETA_THRESHOLD, cos_theta_wo));
    let val: f32 = thickness * (inverse_cos_theta_i + inverse_cos_theta_o);
    let absorption: vec3<f32> = exp(-inverse_color * val);
    return absorption;
}

fn get_ggx_alpha(roughness_factor: f32) -> f32 {
    let roughness: f32 = max(MIN_ROUGHNESS_THRESHOLD, roughness_factor);
    return sqr(roughness);
}

fn fresnel_schlick_basic(w_dot_microfacet_n: f32) -> vec3<f32> {
    let f0 = vec3<f32>(0.04);
    return f0 + (1.0 - f0) * pow(max(0.0, 1.0 - w_dot_microfacet_n), 5.0);
}

fn fresnel_schlick(f0: vec3<f32>, f90: vec3<f32>, w_dot_microfacet_n: f32) -> vec3<f32> {
    return f0 + (f90 - f0) * pow(max(0.0, 1.0 - w_dot_microfacet_n), 5.0);
}

fn fresnel_dielectric(cos_theta: f32, ni: f32, nt: f32) -> f32 {
    let nt2 = sqr(nt);
    let ni2 = sqr(ni);
    let cos_theta2 = sqr(cos_theta);

    let sin_theta2 = 1.0 - cos_theta2;
    let sin_theta = safe_sqrt(sin_theta2);
    let tan_theta = sin_theta / cos_theta;
    let tan_theta2 = sqr(tan_theta);

    let sq = nt2 - ni2 * sin_theta2;
    let sqr = sqrt(sqr(sq));
    let one_over_2_ni_sq = 1.0 / (2.0 * ni2);
    let ni2_sin_theta2 = ni2 * sin_theta2;

    let a2 = one_over_2_ni_sq * (sqr + (nt2 - ni2_sin_theta2));
    let b2 = one_over_2_ni_sq * (sqr - (nt2 - ni2_sin_theta2));
    let a = sqrt(a2);
    let ax2 = a * 2.0;

    let a2_plus_b2 = a2 + b2;

    let rs_numer = a2_plus_b2 - ax2 * cos_theta + cos_theta2;
    let rs_denom = a2_plus_b2 + ax2 * cos_theta + cos_theta2;
    let rs = rs_numer / rs_denom;

    let rp_numer = a2_plus_b2 - (ax2 * sin_theta * tan_theta) + sin_theta2 * tan_theta2;
    let rp_denom = a2_plus_b2 + (ax2 * sin_theta * tan_theta) + sin_theta2 * tan_theta2;
    let rp = rs * (rp_numer / rp_denom);

    return 0.5 * (rp + rs);
}

fn polynomial_fit(v: f32, c0: vec3<f32>, c1: vec3<f32>, c2: vec3<f32>) -> vec3<f32> {
    return c2 * pow(v, 2.0) + c1 * v + c0;
}

fn fresnel_conductor_fitted(cos_theta: f32, ni: f32) -> vec3<f32> {
    let ni01: f32 = (min(ni, 2.5) - 1.0) / 1.5;

    var unpacked0: vec2<f32> = unpack2x16float(_FE[0 * 4 + 0]);
    var unpacked1: vec2<f32> = unpack2x16float(_FE[0 * 4 + 1]);
    var unpacked2: vec2<f32> = unpack2x16float(_FE[0 * 4 + 2]);
    var unpacked3: vec2<f32> = unpack2x16float(_FE[0 * 4 + 3]);
    var unpacked4: vec2<f32> = unpack2x16float(_FE[1 * 4 + 0]);
    let pwr: vec3<f32> = polynomial_fit(ni01,
        vec3<f32>(unpacked0, unpacked1.x),
        vec3<f32>(unpacked1.y, unpacked2),
        vec3<f32>(unpacked3, unpacked4.x),
    );

    unpacked0 = unpacked4;
    unpacked1 = unpack2x16float(_FE[1 * 4 + 1]);
    unpacked2 = unpack2x16float(_FE[1 * 4 + 2]);
    unpacked3 = unpack2x16float(_FE[1 * 4 + 3]);
    unpacked4 = unpack2x16float(_FE[2 * 4 + 0]);
    let a: vec3<f32> = polynomial_fit(ni01,
        vec3<f32>(unpacked0.y, unpacked1),
        vec3<f32>(unpacked2, unpacked3.x),
        vec3<f32>(unpacked3.y, unpacked4),
    );

    unpacked0 = unpack2x16float(_FE[2 * 4 + 1]);
    unpacked1 = unpack2x16float(_FE[2 * 4 + 2]);
    unpacked2 = unpack2x16float(_FE[2 * 4 + 3]);
    unpacked3 = unpack2x16float(_FE[3 * 4 + 0]);
    unpacked4 = unpack2x16float(_FE[3 * 4 + 1]);
    let f0: vec3<f32> = polynomial_fit(ni01,
        vec3<f32>(unpacked0, unpacked1.x),
        vec3<f32>(unpacked1.y, unpacked2),
        vec3<f32>(unpacked3, unpacked4.x),
    );

    let v: vec3<f32> = a * cos_theta * pow(vec3<f32>(1.0 - cos_theta), pwr);

    return saturate(f0 + (1.0 - f0) * pow(1.0 - cos_theta, 5.0) - v);
}

fn eval_ggx_g1(n_dot_wo: f32, ggx_alpha: f32) -> f32 {
    if (n_dot_wo <= 0.0) {
        return 0.0;
    }
    let a2: f32 = sqr(ggx_alpha);
    let denom_c: f32 = sqrt(a2 + (1.0 - a2) * sqr(n_dot_wo)) + n_dot_wo;
    return (2.0 * n_dot_wo) / denom_c;
}

fn eval_ggx_g2(n_dot_wi: f32, n_dot_wo: f32, ggx_alpha: f32) -> f32 {
    let a2: f32 = sqr(ggx_alpha);
    let denom_a: f32 = n_dot_wo * sqrt(a2 + (1.0 - a2) * sqr(n_dot_wi));
    let denom_b: f32 = n_dot_wi * sqrt(a2 + (1.0 - a2) * sqr(n_dot_wo));
    return (2.0 * n_dot_wi * n_dot_wo) / (denom_a + denom_b);
}

fn sample_ggx_vndf(w_out_tangent_space: vec3<f32>, ggx_alpha: f32, uv: vec2<f32>) -> vec3<f32> {
    let alpha = vec2<f32>(ggx_alpha);

    let w_out_hemisphere: vec3<f32> = normalize(vec3<f32>(
        w_out_tangent_space.x * alpha.x,
        w_out_tangent_space.y * alpha.y,
        w_out_tangent_space.z
    ));

    let phi: f32 = TWO_PI * uv.x;
    let a: f32 = saturate(min(alpha.x, alpha.y));
    let s: f32 = 1.0 + length(w_out_tangent_space.xy);
    let a2: f32 = sqr(a);
    let s2: f32 = sqr(s);
    let k: f32 = (1.0 - a2) * s2 / (s2 + a2 * sqr(w_out_tangent_space.z));
    var b: f32;
    if (w_out_tangent_space.z > 0.0) { // TODO: use mix
        b = k * w_out_hemisphere.z;
    } else {
        b = w_out_hemisphere.z;
    }

    let z: f32 = (1.0 - uv.y) * (1.0 + b) - b;
    let sin_theta: f32 = safe_sqrt(saturate(1.0 - sqr(z)));
    let w_in_hemisphere = vec3<f32>(sin_theta * cos(phi), sin_theta * sin(phi), z);
    let microfacet_hemisphere: vec3<f32> = w_out_hemisphere + w_in_hemisphere;

    return normalize(vec3<f32>(
        microfacet_hemisphere.xy * alpha,
        max(0.0, microfacet_hemisphere.z)
    ));
}

fn eval_ggx_ndf(ggx_alpha: f32, microfacet_normal: vec3<f32>) -> f32 {
    if (microfacet_normal.z <= 0.0) {
        return 0.0;
    }

    let a2: f32 = sqr(ggx_alpha);
    var cos2_theta: f32;
    let squared_part_of_denom: f32 = (a2 + tan2ThetaTangentSpaceIntermediate(microfacet_normal, &cos2_theta));
    return a2 / (PI * sqr(cos2_theta) * sqr(squared_part_of_denom));
}

fn eval_ggx_vndf_pdf(ggx_alpha: f32, w_out_tangent_space: vec3<f32>, microfacet_normal: vec3<f32>) -> f32 {
    let alpha = vec2<f32>(ggx_alpha);
    let ndf: f32 = eval_ggx_ndf(ggx_alpha, microfacet_normal);
    let a_o: vec2<f32> = alpha * w_out_tangent_space.xy;
    let len2: f32 = dot(a_o, a_o);
    let t: f32 = safe_sqrt(len2 + sqr(w_out_tangent_space.z));

    if (w_out_tangent_space.z > 0.0) {
        let a: f32 = saturate(min(alpha.x, alpha.y));
        let s: f32 = 1.0 + length(w_out_tangent_space.xy);
        let a2: f32 = sqr(a);
        let s2: f32 = sqr(s);
        let k: f32 = (1.0 - a2) * s2 / (s2 + a2 * sqr(w_out_tangent_space.z));
        return ndf / (2.0 * (k * w_out_tangent_space.z + t));
    }

    return ndf * (t - w_out_tangent_space.z) / (2.0 * len2);
}