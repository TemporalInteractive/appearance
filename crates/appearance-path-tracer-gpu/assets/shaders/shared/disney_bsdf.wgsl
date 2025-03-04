/* disney2.h - License information:

   This code has been adapted from AppleSeed: https://appleseedhq.net
   The AppleSeed software is released under the MIT license.
   Copyright (c) 2014-2018 Esteban Tovagliari, The appleseedhq Organization.

   Permission is hereby granted, free of charge, to any person obtaining a copy
   of this software and associated documentation files (the "Software"), to deal
   in the Software without restriction, including without limitation the rights
   to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
   copies of the Software, and to permit persons to whom the Software is
   furnished to do so, subject to the following conditions:

   The above copyright notice and this permission notice shall be included in
   all copies or substantial portions of the Software.
*/

@include appearance-path-tracer-gpu::shared/bsdf
@include appearance-path-tracer-gpu::shared/ggxmdf
@include appearance-path-tracer-gpu::shared/material_pool

fn world_2_tangent(v: vec3<f32>, n: vec3<f32>, t: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(v, t),
        dot(v, b),
        dot(v, n)
    );
}

fn tangent_2_world(v: vec3<f32>, n: vec3<f32>, t: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    return v.x * t + v.y * b + v.z * n;
}

fn schlick_fresnel(u: f32) -> f32 {
    let m: f32 = clamp(1.0 - u, 0.0, 1.0);
    let m4: f32 = sqr(sqr(m));
    return m4 * m;
}

fn fresnel_reflectance_dielectric(eta: f32, cos_theta_i: f32, cos_theta_t: f32) -> f32 {
    if (cos_theta_i == 0.0 && cos_theta_t == 0.0) {
        return 1.0;
    }

    let k0: f32 = eta * cos_theta_t;
    let k1: f32 = eta * cos_theta_i;
    return 0.5 * (sqr((cos_theta_i - k0) / (cos_theta_i + k0)) + sqr((cos_theta_t - k1) / (cos_theta_t + k1)));
}

fn fresnel_reflectance(cos_theta_i: f32, eta: f32, cos_theta_t: ptr<function, f32>) -> f32 {
    let sin_theta_t2: f32 = (1.0 - sqr(cos_theta_i)) * sqr(eta);
    if (sin_theta_t2 > 1.0) {
        *cos_theta_t = 0.0;
        return 1.0;
    }

    *cos_theta_t = min(sqrt(max(1.0 - sin_theta_t2, 0.0)), 1.0);
    return fresnel_reflectance_dielectric(eta, abs(cos_theta_i), *cos_theta_t);
}

fn evaluate_reflection(reflection_color: vec3<f32>, wo: vec3<f32>, wi: vec3<f32>, m: vec3<f32>, ggx_mdf: GgxMdf, f: f32) -> vec3<f32> {
    let denom: f32 = abs(4.0 * wo.z * wi.z);
    if (denom == 0.0) {
        return vec3<f32>(0.0);
    }

    let d: f32 = GgxMdf::d(ggx_mdf, m);
    let g: f32 = GgxMdf::g(ggx_mdf, wi, wo, m);
    return reflection_color * (f * d * g / denom);
}

fn evaluate_refraction(eta: f32, reflection_color: vec3<f32>, adjoint: bool, wo: vec3<f32>, wi: vec3<f32>, m: vec3<f32>, ggx_mdf: GgxMdf, t: f32) -> vec3<f32> {
    if (wo.z == 0.0 || wi.z == 0.0) {
        return vec3<f32>(0.0);
    }

    let cos_ih: f32 = dot(m, wi);
    let cos_oh: f32 = dot(m, wo);
    let dots: f32 = (cos_ih * cos_oh) / (wi.z * wo.z);
    let sqrt_denom: f32 = cos_oh + eta * cos_ih;
    if (abs(sqrt_denom) < 1e-6) {
        return vec3<f32>(0.0);
    }

    let d: f32 = GgxMdf::d(ggx_mdf, m);
    let g: f32 = GgxMdf::g(ggx_mdf, wi, wo, m);
    var multiplier: f32 = abs(dots) * t * d * g / sqr(sqrt_denom);
    if (!adjoint) {
        multiplier *= sqr(eta);
    }
    return reflection_color * multiplier;
}

fn reflection_jacobian(cos_oh: f32) -> f32 {
    if (cos_oh == 0.0) {
        return 0.0;
    }

    return 1.0 / (4.0 * abs(cos_oh));
}

fn refraction_jacobian(wo: vec3<f32>, wi: vec3<f32>, m: vec3<f32>, eta: f32) -> f32 {
    let cos_ih: f32 = dot(m, wi);
    let cos_oh: f32 = dot(m, wo);
    let sqrt_denom: f32 = cos_oh + eta * cos_ih;
    if (abs(sqrt_denom) < 1e-6) {
        return 0.0;
    }

    return abs(cos_ih) * sqr(eta / sqrt_denom);
}

fn improve_normalization(v: vec3<f32>) -> vec3<f32> {
    return v * ((3.0 - dot(v, v)) * 0.5);
}

fn refracted_direction(wo: vec3<f32>, m: vec3<f32>, cos_wom: f32, cos_theta_t: f32, rcp_eta: f32) -> vec3<f32> {
    var wi: vec3<f32>;
    if (cos_wom > 0.0) {
        wi = (rcp_eta * cos_wom - cos_theta_t) * m - rcp_eta * wo;
    } else {
        wi = (rcp_eta * cos_wom + cos_theta_t) * m - rcp_eta * wo;
    }
    return improve_normalization(wi);
}

fn diffuse_reflection_cos_weighted(r0: f32, r1: f32) -> vec3<f32> {
    let term1: f32 = TWO_PI * r0;
    let term2: f32 = sqrt(1.0 - r1);
    let s: f32 = sin(term1);
    let c: f32 = cos(term1);
    return vec3<f32>(c * term2, s * term2, sqrt(r1));
}

fn mix_spectra(a: vec3<f32>, b: vec3<f32>, t: f32) -> vec3<f32> {
    return (1.0 -  t) * a + t * b;
}

fn mix_one_with_spectra(b: vec3<f32>, t: f32) -> vec3<f32> {
    return vec3<f32>(1.0 - t) + t * b;
}

fn mix_spectra_with_one(a: vec3<f32>, t: f32) -> vec3<f32> {
    return (1.0 - t) * a + vec3<f32>(t);
}

fn force_above_surface(direction: vec3<f32>, normal: vec3<f32>, out_direction: ptr<function, vec3<f32>>) -> bool {
    let cos_theta = dot(direction, normal);
    let correction = 1e-4 - cos_theta;
    if (correction <= 0.0) {
        return false;
    } else {
        *out_direction = normalize(direction + correction * normal);
        return true;
    }
}

fn fr_l(_v_dot_n: f32, _eio: f32) -> f32 {
    var v_dot_n: f32 = _v_dot_n;
    var eio: f32 = _eio;
    if (v_dot_n < 0.0) {
        eio = 1.0 / eio;
        v_dot_n = abs(v_dot_n);
    }

    let sin_theta_t2: f32 = (1.0 - sqr(v_dot_n)) * sqr(eio);
    if (sin_theta_t2 > 1.0) {
        return 1.0;
    }

    let l_dot_n: f32 = min(sqrt(max(0.0, 1.0 - sin_theta_t2)), 1.0);
    let r1: f32 = (v_dot_n - eio * l_dot_n) / (v_dot_n + eio * l_dot_n);
    let r2: f32 = (l_dot_n - eio * v_dot_n) / (l_dot_n + eio * v_dot_n);
    return 0.5 * (sqr(r1) + sqr(r2));
}

fn refract_l(wi: vec3<f32>, n: vec3<f32>, eta: f32, wt: ptr<function, vec3<f32>>) -> bool {
    let cos_theta_i: f32 = abs(dot(n, wi));
    let sin_2_theta_i: f32 = max(0.0, 1.0 - sqr(cos_theta_i));
    let sin_2_theta_t: f32 = sqr(eta) * sin_2_theta_i;
    if (sin_2_theta_t >= 1.0) {
        return false;
    }

    let cos_theta_t: f32 = sqrt(1.0 - sin_2_theta_t);
    *wt = eta * (wi * -1.0) + (eta * cos_theta_i - cos_theta_t) * n;
    return true;
}

struct DisneyBsdf {
    color: vec3<f32>,
    metallic: f32,
    transmittance: vec3<f32>,
    subsurface: f32,
    luminance: f32,
    specular: f32,
    roughness: f32,
    spec_tint: vec3<f32>,
    anisotropic: f32,
    sheen: f32,
    sheen_tint: vec3<f32>,
    clearcoat: f32,
    clearcoat_gloss: f32,
    transmission: f32,
    eta: f32,
};

fn DisneyBsdf::from_material(material: Material) -> DisneyBsdf {
    var bsdf: DisneyBsdf;
    bsdf.color = material.color;
    bsdf.metallic = material.metallic;
    bsdf.transmittance = material.absorption;
    bsdf.subsurface = material.subsurface;
    bsdf.luminance = material.luminance;
    bsdf.specular = max(material.specular, 1.0 - material.roughness);
    bsdf.roughness = material.roughness;
    bsdf.spec_tint = material.specular_tint;
    bsdf.anisotropic = material.anisotropic;
    bsdf.sheen = material.sheen;
    bsdf.sheen_tint = material.sheen_tint;
    bsdf.clearcoat = material.clearcoat;
    bsdf.clearcoat_gloss = 1.0 - material.clearcoat_roughness;
    bsdf.transmission = material.transmission;
    bsdf.eta = material.eta;
    return bsdf;
}

fn DisneyBsdf::clearcoat_roughness(_self: DisneyBsdf) -> f32 {
    return mix(0.1, 0.001, _self.clearcoat_gloss);
}

fn DisneyBsdf::specular_fresnel(_self: DisneyBsdf, o: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    var value: vec3<f32> = _self.spec_tint;
    value *= _self.specular * 0.08;
    value = mix_spectra(value, _self.color, _self.metallic);
    let cos_oh: f32 = abs(dot(o, h));
    return mix_spectra_with_one(value, schlick_fresnel(cos_oh));
}

fn DisneyBsdf::clearcoat_fresnel(_self: DisneyBsdf, o: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    let cos_oh: f32 = abs(dot(o, h));
    return vec3<f32>(mix(0.04, 1.0, schlick_fresnel(cos_oh)) * 0.25 * _self.clearcoat);
}

fn DisneyBsdf::sample_mf(_self: DisneyBsdf, r0: f32, r1: f32, alpha_x: f32, alpha_y: f32, wol: vec3<f32>,
     mf_clearcoat: bool, wil: ptr<function, vec3<f32>>, pdf: ptr<function, f32>, value: ptr<function, vec3<f32>>) {
    if (wol.z == 0.0) {
        *value = vec3<f32>(0.0);
        *pdf = 0.0;
        return;
    }

    if (mf_clearcoat) {
        let gtr1_mdf = Gtr1Mdf::new(alpha_x, alpha_y);

        let m: vec3<f32> = Gtr1Mdf::sample(gtr1_mdf, r0, r1);
        *wil = reflect(wol * -1.0, m);

        if ((*wil).z == 0.0) {
            return;
        }

        let cos_oh: f32 = dot(wol, m);
        *pdf = Gtr1Mdf::pdf(gtr1_mdf, wol, m) / abs(4.0 * cos_oh);
        if (*pdf < 1e-6) {
            return;
        }

        let d: f32 = Gtr1Mdf::d(gtr1_mdf, m);
        let g: f32 = Gtr1Mdf::g(gtr1_mdf, *wil, wol, m);
        *value = DisneyBsdf::clearcoat_fresnel(_self, wol, m);
        *value *= d * g;
    } else {
        let ggx_mdf = GgxMdf::new(alpha_x, alpha_y);

        let m: vec3<f32> = GgxMdf::sample(ggx_mdf, wol, r0, r1);
        *wil = reflect(wol * -1.0, m);

        if ((*wil).z == 0.0) {
            return;
        }

        let cos_oh: f32 = dot(wol, m);
        *pdf = GgxMdf::pdf(ggx_mdf, wol, m) / abs(4.0 * cos_oh);
        if (*pdf < 1e-6) {
            return;
        }

        let d: f32 = GgxMdf::d(ggx_mdf, m);
        let g: f32 = GgxMdf::g(ggx_mdf, *wil, wol, m);
        *value = DisneyBsdf::specular_fresnel(_self, wol, m);
        *value *= d * g;
    }
}

fn DisneyBsdf::evaluate_mf(_self: DisneyBsdf, alpha_x: f32, alpha_y: f32, wol: vec3<f32>,
     wil: vec3<f32>, m: vec3<f32>, mf_clearcoat: bool,
     bsdf: ptr<function, vec3<f32>>) -> f32 {
    if (wol.z == 0.0 || wil.z == 0.0) {
        return 0.0;
    }

    let cos_oh: f32 = dot(wol, m);
    if (cos_oh == 0.0) {
        return 0.0;
    }

    if (mf_clearcoat) {
        let gtr1_mdf = Gtr1Mdf::new(alpha_x, alpha_y);

        let d: f32 = Gtr1Mdf::d(gtr1_mdf, m);
        let g: f32 = Gtr1Mdf::g(gtr1_mdf, wil, wol, m);
        *bsdf = DisneyBsdf::clearcoat_fresnel(_self, wol, m);

        *bsdf *= d * g / abs(4.0 * wol.z * wil.z);
        return Gtr1Mdf::pdf(gtr1_mdf, wol, m) / abs(4.0 * cos_oh);
    } else {
        let ggx_mdf = GgxMdf::new(alpha_x, alpha_y);

        let d: f32 = GgxMdf::d(ggx_mdf, m);
        let g: f32 = GgxMdf::g(ggx_mdf, wil, wol, m);
        *bsdf = DisneyBsdf::specular_fresnel(_self, wol, m);

        *bsdf *= d * g / abs(4.0 * wol.z * wil.z);
        return GgxMdf::pdf(ggx_mdf, wol, m) / abs(4.0 * cos_oh);
    }
}

fn DisneyBsdf::evaluate_diffuse(_self: DisneyBsdf, i_n: vec3<f32>, wow: vec3<f32>, wiw: vec3<f32>,
     m: vec3<f32>, value: ptr<function, vec3<f32>>) -> f32 {
    let cos_on: f32 = dot(i_n, wow);
    let cos_in: f32 = dot(i_n, wiw);
    let cos_ih: f32 = dot(wiw, m);
    let fl: f32 = schlick_fresnel(cos_in);
    let fv: f32 = schlick_fresnel(cos_on);

    var fd: f32 = 0.0;
    if (_self.subsurface != 1.0) {
        let fd90: f32 = 0.5 + 2.0 * sqr(cos_ih) * _self.roughness;
        fd = mix(1.0, fd90, fl) * mix(1.0, fd90, fv);
    }
    if (_self.subsurface > 0.0) {
        let fss90: f32 = sqr(cos_ih) * _self.roughness;
        let fss: f32 = mix(1.0, fss90, fl) * mix(1.0, fss90, fv);
        let ss: f32 = 1.25 * (fss * (1.0 / (abs(cos_on) + abs(cos_in)) - 0.5) + 0.5);
        fd = mix(fd, ss, _self.subsurface);
    }
    *value = _self.color * fd * INV_PI * (1.0 - _self.metallic);
    return abs(cos_in) * INV_PI;
}

fn DisneyBsdf::evaluate_sheen(_self: DisneyBsdf, wow: vec3<f32>, wiw: vec3<f32>, m: vec3<f32>,
     value: ptr<function, vec3<f32>>) -> f32 {
    let h: vec3<f32> = normalize(wow + wiw);
    let cos_ih: f32 = dot(wiw, m);
    let fh: f32 = schlick_fresnel(cos_ih);
    *value = _self.sheen_tint;
    *value *= fh * _self.sheen * (1.0 - _self.metallic);
    return 1.0 / (2.0 * PI);
}

fn DisneyBsdf::sample(_self: DisneyBsdf, i_n: vec3<f32>, tangent_to_world: mat3x3<f32>, world_to_tangent: mat3x3<f32>,
     wow: vec3<f32>, distance: f32, back_face: bool, r0: f32, r1: f32, r2: f32,
     wiw: ptr<function, vec3<f32>>, pdf: ptr<function, f32>, specular: ptr<function, bool>) -> vec3<f32> {
    // TODO: ??
    let adjoint: bool = false;
    
    var flip: f32;
    if (back_face) {
        flip = -1.0;
    } else {
        flip = 1.0;
    }

    if (r0 < _self.transmission) {
        *specular = true;

        let r3: f32 = r0 / _self.transmission;
        let wol: vec3<f32> = world_to_tangent * wow;
        var eta: f32;
        if (flip < 0.0) {
            eta = 1.0 / _self.eta;
        } else {
            eta = _self.eta;
        }
        if (eta == 1.0) {
            return vec3<f32>(0.0);
        }

        let alpha: vec2<f32> = microfacet_alpha_from_roughness(_self.roughness, _self.anisotropic);
        let ggx_mdf = GgxMdf::new(alpha.x, alpha.y);

        let m: vec3<f32> = GgxMdf::sample(ggx_mdf, wol, r1, r3);
        let rcp_eta: f32 = 1.0 / eta;
        let cos_wom: f32 = clamp(dot(wol, m), -1.0, 1.0);

        var cos_theta_t: f32;
        let f: f32 = fresnel_reflectance(cos_wom, eta, &cos_theta_t);

        var jacobian: f32;
        var wil: vec3<f32>;
        var ret_val: vec3<f32>;
        if (r2 < f) {
            wil = reflect(wol * -1.0, m);
            if (wil.z * wol.z <= 0.0) {
                return vec3<f32>(0.0);
            }
            ret_val = evaluate_reflection(_self.color, wol, wil, m, ggx_mdf, f);
            *pdf = f;
            jacobian = reflection_jacobian(cos_wom);
        } else {
            wil = refracted_direction(wol, m, cos_wom, cos_theta_t, eta);
            if (wil.z * wol.z > 0.0) {
                return vec3<f32>(0.0);
            }
            ret_val = evaluate_refraction(rcp_eta, _self.color, adjoint, wol, wil, m, ggx_mdf, 1.0 - f);
            *pdf = 1.0 - f;
            jacobian = refraction_jacobian(wol, wil, m, rcp_eta);
        }
        *pdf *= jacobian * GgxMdf::pdf(ggx_mdf, wol, m);
        if (*pdf > 1e-6) {
            *wiw = tangent_to_world * wil;
        }

        if (back_face) {
            let beer = vec3<f32>(
                exp(-_self.transmittance.x * distance * 2.0),
                exp(-_self.transmittance.y * distance * 2.0),
                exp(-_self.transmittance.z * distance * 2.0)
            );

            ret_val *= beer;
        }

        return ret_val;
    }

    let r3: f32 = (r0 - _self.transmission) / (1.0 - _self.transmission);

    var weights = vec4<f32>(
        mix(_self.luminance, 0.0, _self.metallic),
        mix(_self.sheen, 0.0, _self.metallic),
        mix(_self.specular, 1.0, _self.metallic),
        _self.clearcoat * 0.25
    );
    weights *= 1.0 / (weights.x + weights.y + weights.z + weights.w);
    let cdf = vec4<f32>(
        weights.x,
        weights.x + weights.y,
        weights.x + weights.y + weights.z,
        0.0
    );

    var probability: f32;
    var component_pdf: f32;
    var value = vec3<f32>(0.0);
    if (r3 < cdf.y) {
        let r2: f32 = r3 / cdf.y;
        *wiw = tangent_to_world * diffuse_reflection_cos_weighted(r2, r1);
        let m: vec3<f32> = normalize(*wiw + wow);

        if (r3 < cdf.x) {
            component_pdf = DisneyBsdf::evaluate_diffuse(_self, i_n, wow, *wiw, m, &value);
            probability = weights.x * component_pdf;
            weights.x = 0.0;
        } else {
            component_pdf = DisneyBsdf::evaluate_sheen(_self, wow, *wiw, m, &value);
            probability = weights.y * component_pdf;
            weights.y = 0.0;
        }
    } else {
        let wol: vec3<f32> = world_to_tangent * wow;
        var wil: vec3<f32>;
        if (r3 < cdf.z) {
            let r2: f32 = (r3 - cdf.y) / (cdf.z - cdf.y);
            let alpha: vec2<f32> = microfacet_alpha_from_roughness(_self.roughness, _self.anisotropic);
            DisneyBsdf::sample_mf(_self, r2, r1, alpha.x, alpha.y, wol, false, &wil, &component_pdf, &value);
            probability = weights.z * component_pdf;
            weights.z = 0.0;
        } else {
            let r2: f32 = (r3 - cdf.z) / (1.0 - cdf.z);
            let alpha: f32 = DisneyBsdf::clearcoat_roughness(_self);
            DisneyBsdf::sample_mf(_self, r2, r1, alpha, alpha, wol, true, &wil, &component_pdf, &value);
            probability = weights.w * component_pdf;
            weights.w = 0.0;
        }
        value *= 1.0 / abs(4.0 * wol.z * wil.z);
        *wiw = tangent_to_world * wil;
    }

    var contrib: vec3<f32>;
    if (weights.x + weights.y > 0.0) {
        let m: vec3<f32> = normalize(*wiw + wow);
        if (weights.x > 0.0) {
            probability += weights.x * DisneyBsdf::evaluate_diffuse(_self, i_n, wow, *wiw, m, &contrib);
            value += contrib;
        }
        if (weights.y > 0.0) {
            probability += weights.y * DisneyBsdf::evaluate_sheen(_self, wow, *wiw, m, &contrib);
            value += contrib;
        }
    }

    if (weights.z + weights.w > 0.0) {
        let wol: vec3<f32> = world_to_tangent * wow;
        let wil: vec3<f32> = world_to_tangent * (*wiw);
        let m: vec3<f32> = normalize(wol + wil);
        if (weights.z > 0.0) {
            let alpha: vec2<f32> = microfacet_alpha_from_roughness(_self.roughness, _self.anisotropic);
            probability += weights.z * DisneyBsdf::evaluate_mf(_self, alpha.x, alpha.y, wol, wil, m, false, &contrib);
            value += contrib;
        }
        if (weights.w > 0.0) {
            let alpha: f32 = DisneyBsdf::clearcoat_roughness(_self);
            probability += weights.w * DisneyBsdf::evaluate_mf(_self, alpha, alpha, wol, wil, m, true, &contrib);
            value += contrib;
        }
    }

    if (probability > 1e-6) {
        *pdf = probability;
    } else {
        *pdf = 0.0;
    }
    return value;
}