@include appearance-path-tracer-gpu::shared/material_pool
@include ::color

@group(2)
@binding(0)
var<storage, read> material_descriptors: array<MaterialDescriptor>;

@group(2)
@binding(1)
var material_textures: binding_array<texture_2d<f32>, MAX_MATERIAL_POOL_TEXTURES>;

@group(2)
@binding(2)
var material_texture_sampler: sampler;

fn _texture(id: u32, tex_coord: vec2<f32>) -> vec4<f32> {
    return textureSampleLevel(material_textures[id], material_texture_sampler, tex_coord, 0.0);
}

fn MaterialDescriptor::base_color(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec4<f32> {
    var base_color: vec4<f32> = _self.base_color_factor;
    if (_self.base_color_texture != INVALID_TEXTURE) {
        base_color *= _texture(_self.base_color_texture, tex_coord);
    }
    return srgb_to_linear(base_color);
}

fn MaterialDescriptor::occlusion(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> f32 {
    var occlusion: f32 = 1.0;
    if (_self.occlusion_texture != INVALID_TEXTURE) {
        occlusion *= mix(1.0, _texture(_self.occlusion_texture, tex_coord).r, _self.occlusion_strength);
    }
    return occlusion;
}

fn MaterialDescriptor::emission(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec3<f32> {
    var emission: vec3<f32> = _self.emissive_factor;
    if (_self.emissive_texture != INVALID_TEXTURE) {
        emission *= _texture(_self.emissive_texture, tex_coord).rgb;
    }
    return emission;
}

fn MaterialDescriptor::metallic_roughness(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec2<f32> {
    var metallic: f32 = _self.metallic_factor;
    var roughness: f32 = _self.roughness_factor;
    if (_self.metallic_roughness_texture != INVALID_TEXTURE) {
        var mr: vec3<f32> = _texture(_self.metallic_roughness_texture, tex_coord).rgb;
        metallic *= mr.b;
        roughness *= mr.g;
    }
    return vec2<f32>(metallic, roughness);
}

fn Material::from_material_descriptor(material_descriptor: MaterialDescriptor, tex_coord: vec2<f32>) -> Material {
    var material: Material;
    material.base_color = MaterialDescriptor::base_color(material_descriptor, tex_coord);
    material.occlusion = MaterialDescriptor::occlusion(material_descriptor, tex_coord);
    let metallic_roughness = MaterialDescriptor::metallic_roughness(material_descriptor, tex_coord);
    material.metallic = metallic_roughness.x;
    material.roughness = metallic_roughness.y;
    material.ior = material_descriptor.ior;
    material.transmission = material_descriptor.transmission_factor;
    material.emission = MaterialDescriptor::emission(material_descriptor, tex_coord);
    material.alpha_cutoff = material_descriptor.alpha_cutoff;
    return material;
}