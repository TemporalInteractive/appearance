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
    cleatcoat_gloss: f32,
    transmission: f32,
    eta: f32,
    _padding0: u32,
    _padding01: u32,
    _padding2: u32,
};

fn DisneyBsdf::clearcoat_roughness(_self: DisneyBsdf) -> f32 {
    return mix(0.1, 0.001, _self.cleatcoat_gloss);
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