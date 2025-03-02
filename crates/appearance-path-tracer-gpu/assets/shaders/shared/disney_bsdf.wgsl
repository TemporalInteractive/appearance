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

fn schlick_fresnel(u: f32) -> f32 {
    let m: f32 = clamp(1.0 - u, 0.0, 1.0);
    let m4: f32 = sqr(sqr(m));
    return m4 * m;
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
    tint: vec3<f32>,
    specular: f32,
    roughness: f32,
    spec_tint: f32,
    anisotropic: f32,
    sheen: f32,
    sheen_tint: f32,
    clearcoat: f32,
    clearcoat_gloss: f32,
    transmission: f32,
    eta: f32,
    _padding0: u32,
    _padding01: u32,
    _padding2: u32,
};

fn DisneyBsdf::from_material(material: Material) -> DisneyBsdf {
    var bsdf: DisneyBsdf;
    bsdf.color = material.base_color.rgb;
    bsdf.metallic = material.metallic;
    bsdf.transmittance = vec3<f32>(1.0);
    bsdf.subsurface = 0.0;
    bsdf.tint = vec3<f32>(0.0);
    bsdf.specular = 0.0;
    bsdf.roughness = max(0.001, material.roughness);
    bsdf.spec_tint = 0.0;
    bsdf.clearcoat = 0.0;
    bsdf.clearcoat_gloss = 0.0;
    bsdf.transmission = material.transmission;
    bsdf.eta = material.ior;
    return bsdf;
}

fn DisneyBsdf::clearcoat_roughness(_self: DisneyBsdf) -> f32 {
    return mix(0.1, 0.001, _self.clearcoat_gloss);
}

fn DisneyBsdf::specular_fresnel(_self: DisneyBsdf, o: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    var value: vec3<f32> = mix_one_with_spectra(_self.tint, _self.spec_tint);
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
    *value = mix_one_with_spectra(_self.tint, _self.sheen_tint);
    *value *= fh * _self.sheen * (1.0 - _self.metallic);
    return 1.0 / (2.0 * PI);
}

// fn DisneyBsdf::sample(_self: DisneyBsdf, _i_n: vec3<f32>, n: vec3<f32>, i_t: vec3<f32>,
//      wow: vec3<f32>, distance: f32, r0: f32, r1: f32, r2: f32,
//      wiw: ptr<function, vec3<f32>>, pdf: ptr<function, f32>, specular: ptr<function, bool>) -> vec3<f32> {
//     // TODO: this flip should also not be necessary
//     var flip: f32;
//     if (dot(wow, n) < 0.0) {
//         flip = -1.0;
//     } else {
//         flip = 1.0;
//     }
//     let i_n: vec3<f32> = _i_n * flip;

//     // TODO: we shouldn't have to recalculate the tangent matrix, already precomputed
//     let b: vec3<f32> = normalize(cross(i_n, i_t));
//     let t: vec3<f32> = normalize(cross(i_n, b));

//     if (r0 < _self.transmission) {
//         *specular = true;

//         let r3: f32 = r0 / _self.transmission;
//         let wol: vec3<f32> = 
//     }
// }