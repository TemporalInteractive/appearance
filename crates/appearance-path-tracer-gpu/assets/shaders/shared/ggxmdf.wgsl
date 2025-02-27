/* ggxmdf.h - License information:

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

@include ::math

struct GgxMdf {
    alpha_x: f32,
    alpha_y: f32,
};

fn GgxMdf::new(alpha_x: f32, alpha_y: f32) -> GgxMdf {
    return GgxMdf(alpha_x, alpha_y);
}

fn GgxMdf::d(_self: GgxMdf, m: vec3<f32>) -> f32 {
    if (m.z == 0.0) {
        return sqr(_self.alpha_x) * INV_PI;
    }

    let cos_theta_2: f32 = sqr(m.z);
    let sin_theta: f32 = sqrt(max(0.0, 1.0 - cos_theta_2));
    let tan_theta_2: f32 = (1.0 - cos_theta_2) / cos_theta_2;

    var stretched_roughness: f32;
    if (_self.alpha_x == _self.alpha_y || sin_theta == 0.0) {
        stretched_roughness = 1.0 / sqr(_self.alpha_x);
    } else {
        stretched_roughness = sqr(m.x / (sin_theta * _self.alpha_x)) + sqr(m.y / (sin_theta * _self.alpha_y));
    }

    return 1.0 / (PI * _self.alpha_x * _self.alpha_y * sqr(cos_theta_2) * sqr(1.0 + tan_theta_2 * stretched_roughness));
}

fn GgxMdf::lambda(_self: GgxMdf, v: vec3<f32>) -> f32 {
    if (v.z == 0.0) {
        return 0.0;
    }

    let cos_theta_2: f32 = sqr(v.z);
    let sin_theta: f32 = sqrt(max(0.0, 1.0 - cos_theta_2));

    var projected_roughness: f32;
    if (_self.alpha_x == _self.alpha_y || sin_theta == 0.0) {
        projected_roughness = _self.alpha_x;
    } else {
        projected_roughness = sqrt(sqr((v.x * _self.alpha_x) / sin_theta) + sqr((v.y * _self.alpha_y) / sin_theta));
    }

    let tan_theta_2: f32 = sqr(sin_theta) / cos_theta_2;
    let a2_rcp: f32 = sqr(projected_roughness) * tan_theta_2;
    return (-1.0 + sqrt(1.0 + a2_rcp)) * 0.5;
}

// TODO both g and g1 have m unused?
fn GgxMdf::g(_self: GgxMdf, wi: vec3<f32>, wo: vec3<f32>, m: vec3<f32>) -> f32 {
    return 1.0 / (1.0 + GgxMdf::lambda(_self, wo) + GgxMdf::lambda(_self, wi));
}

fn GgxMdf::g1(_self: GgxMdf, v: vec3<f32>, m: vec3<f32>) -> f32 {
    return 1.0 / (1.0 + GgxMdf::lambda(_self, v));
}

fn GgxMdf::sample(_self: GgxMdf, v: vec3<f32>, r0: f32, r1: f32) -> vec3<f32> {
    var sign_cos_vn: f32;
    if (v.z < 0.0) {
        sign_cos_vn = -1.0;
    } else {
        sign_cos_vn = 1.0;
    }

    let stretched: vec3<f32> = normalize(vec3<f32>(
        sign_cos_vn * v.x * _self.alpha_x,
        sign_cos_vn * v.y * _self.alpha_y,
        sign_cos_vn * v.z
    ));

    var t1: vec3<f32>;
    if (v.z < 0.9999) {
        t1 = normalize(cross(stretched, vec3<f32>(0.0, 0.0, 1.0)));
    } else {
        t1 = vec3<f32>(1.0, 0.0, 0.0);
    }
    let t2: vec3<f32> = cross(t1, stretched);

    let a: f32 = 1.0 / (1.0 + stretched.z);
    let r: f32 = sqrt(r0);
    var phi: f32;
    if (r1 < a) {
        phi = r1 / a * PI;
    } else {
        phi = PI + (r1 - a) / (1.0 - a) * PI;
    }

    let p1: f32 = r * cos(phi);
    var p2: f32 = r * sin(phi);
    if (r1 >= a) {
        p2 *= stretched.z;
    }

    let h: vec3<f32> = p1 * t1 + p2 * t2 + sqrt(max(0.0, 1.0 - sqr(p1) - sqr(p2))) * stretched;
    return normalize(vec3<f32>(
        h.x * _self.alpha_x,
        h.y * _self.alpha_y,
        max(0.0, h.z)
    ));
}

fn GgxMdf::pdf(_self: GgxMdf, v: vec3<f32>, m: vec3<f32>) -> f32{
    let cos_theta_v: f32 = v.z;
    if (cos_theta_v == 0.0) {
        return 0.0;
    }
    return GgxMdf::g1(_self, v, m) * abs(dot(v, m)) * GgxMdf::d(_self, m) / abs(cos_theta_v);
}