fn linear_to_srgb(linear: vec4<f32>) -> vec4<f32> {
    let cutoff = linear.rgb < vec3<f32>(0.0031308);
    let higher = vec3<f32>(1.055) * pow(linear.rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    let lower = linear.rgb * vec3<f32>(12.92);
    
    return vec4<f32>(select(higher, lower, cutoff), linear.a);
}

fn srgb_to_linear(srgb: vec4<f32>) -> vec4<f32> {
    let cutoff = srgb.rgb < vec3<f32>(0.04045);
    let higher = pow((srgb.rgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    let lower = srgb.rgb / vec3<f32>(12.92);
    
    return vec4<f32>(select(higher, lower, cutoff), srgb.a);
}