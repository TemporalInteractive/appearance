@include ::color
@include appearance-path-tracer-gpu::shared/material/material_pool

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

fn MaterialDescriptor::color(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec4<f32> {
    var color = vec4<f32>(_self.color, 1.0);
    if (_self.color_texture != INVALID_TEXTURE) {
        color *= srgb_to_linear(_texture(_self.color_texture, tex_coord));
    }
    return color;
}

fn MaterialDescriptor::emission(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec3<f32> {
    var emission: vec3<f32> = _self.emission;
    if (_self.emission_texture != INVALID_TEXTURE) {
        emission *= _texture(_self.emission_texture, tex_coord).rgb;
    }
    return emission;
}

fn MaterialDescriptor::metallic_roughness(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec2<f32> {
    var metallic: f32 = _self.metallic;
    var roughness: f32 = _self.roughness;
    if (_self.metallic_roughness_texture != INVALID_TEXTURE) {
        var mr: vec3<f32> = _texture(_self.metallic_roughness_texture, tex_coord).rgb;
        metallic *= mr.b;
        roughness *= mr.g;
    }
    return vec2<f32>(metallic, roughness);
}

fn MaterialDescriptor::clearcoat(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> f32 {
    var clearcoat: f32 = _self.clearcoat;
    if (_self.clearcoat_texture != INVALID_TEXTURE) {
        clearcoat *= _texture(_self.clearcoat_texture, tex_coord).r;
    }
    return clearcoat;
}

fn MaterialDescriptor::clearcoat_roughness(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> f32 {
    var clearcoat_roughness: f32 = _self.clearcoat_roughness;
    if (_self.clearcoat_roughness_texture != INVALID_TEXTURE) {
        clearcoat_roughness *= _texture(_self.clearcoat_roughness_texture, tex_coord).g;
    }
    return clearcoat_roughness;
}

fn MaterialDescriptor::transmission(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> f32 {
    var transmission: f32 = _self.transmission;
    if (_self.transmission_texture != INVALID_TEXTURE) {
        transmission *= _texture(_self.transmission_texture, tex_coord).r;
    }
    return transmission;
}

fn MaterialDescriptor::sheen(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> f32 {
    var sheen: f32 = _self.sheen;
    if (_self.sheen_texture != INVALID_TEXTURE) {
        sheen *= _texture(_self.sheen_texture, tex_coord).r;
    }
    return sheen;
}

fn MaterialDescriptor::sheen_tint(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec3<f32> {
    var sheen_tint: vec3<f32> = _self.sheen_tint;
    if (_self.sheen_tint_texture != INVALID_TEXTURE) {
        sheen_tint *= srgb_to_linear(_texture(_self.sheen_tint_texture, tex_coord)).rgb;
    }
    return sheen_tint;
}

fn MaterialDescriptor::normal_ts(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec3<f32> {
    if (_self.normal_texture == INVALID_TEXTURE) {
        return vec3<f32>(0.0);
    } else {
        let normal_ts: vec3<f32> = _texture(_self.normal_texture, tex_coord).rgb * 2.0 - 1.0;
        return normal_ts;
    }
}

fn MaterialDescriptor::clearcoat_normal_ts(_self: MaterialDescriptor, tex_coord: vec2<f32>) -> vec3<f32> {
    if (_self.clearcoat_normal_texture == INVALID_TEXTURE) {
        return vec3<f32>(0.0);
    } else {
        let normal_ts: vec3<f32> = _texture(_self.clearcoat_normal_texture, tex_coord).rgb * 2.0 - 1.0;
        return normal_ts;
    }
}

fn MaterialDescriptor::apply_normal_mapping(_self: MaterialDescriptor, tex_coord: vec2<f32>, normal_ws: vec3<f32>, hit_tangent_to_world: mat3x3<f32>) -> vec3<f32> {
    if (_self.normal_texture != INVALID_TEXTURE && _self.normal_scale > 0.0) {
        let normal_ts: vec3<f32> = MaterialDescriptor::normal_ts(_self, tex_coord);
        var normal: vec3<f32> = normalize(hit_tangent_to_world * normal_ts);
        if (_self.normal_scale < 1.0) {
            normal = normalize(mix(normal_ws, normal, _self.normal_scale));
        }
        return normal;
    }

    return normal_ws;
}

fn MaterialDescriptor::apply_clearcoat_normal_mapping(_self: MaterialDescriptor, tex_coord: vec2<f32>, normal_ws: vec3<f32>, hit_tangent_to_world: mat3x3<f32>) -> vec3<f32> {
    if (_self.clearcoat_normal_texture != INVALID_TEXTURE) {
        let normal_ts: vec3<f32> = MaterialDescriptor::clearcoat_normal_ts(_self, tex_coord);
        var normal: vec3<f32> = normalize(hit_tangent_to_world * normal_ts);
        return normal;
    }

    return normal_ws;
}

fn Material::from_material_descriptor_with_color(material_descriptor: MaterialDescriptor, tex_coord: vec2<f32>, color: vec4<f32>) -> Material {
    var material: Material;
    material.color = color.rgb;
    material.luminance = color.a;
    let metallic_roughness = MaterialDescriptor::metallic_roughness(material_descriptor, tex_coord);
    material.metallic = metallic_roughness.x;
    material.roughness = metallic_roughness.y;
    material.emission = MaterialDescriptor::emission(material_descriptor, tex_coord);
    material.transmission = MaterialDescriptor::transmission(material_descriptor, tex_coord);
    material.eta = material_descriptor.eta;
    material.subsurface = material_descriptor.subsurface;
    material.absorption = material_descriptor.absorption;
    material.specular = material_descriptor.specular;
    material.specular_tint = material_descriptor.specular_tint;
    material.anisotropic = material_descriptor.anisotropic;
    material.sheen = MaterialDescriptor::sheen(material_descriptor, tex_coord);
    material.sheen_tint = MaterialDescriptor::sheen_tint(material_descriptor, tex_coord);
    material.clearcoat = MaterialDescriptor::clearcoat(material_descriptor, tex_coord);
    material.clearcoat_roughness = MaterialDescriptor::clearcoat_roughness(material_descriptor, tex_coord);
    material.alpha_cutoff = material_descriptor.alpha_cutoff;
    return material;
}

fn Material::from_material_descriptor(material_descriptor: MaterialDescriptor, tex_coord: vec2<f32>) -> Material {
    let color: vec4<f32> = MaterialDescriptor::color(material_descriptor, tex_coord);
    return Material::from_material_descriptor_with_color(material_descriptor, tex_coord, color);
}