@include ::math

const INVALID_TEXTURE: u32 = U32_MAX;
const MAX_MATERIAL_POOL_TEXTURES: u32 = 1024u;

struct MaterialDescriptor {
    color: vec3<f32>,
    color_texture: u32,
    metallic: f32,
    roughness: f32,
    metallic_roughness_texture: u32,
    normal_scale: f32,
    emission: vec3<f32>,
    normal_texture: u32,
    emission_texture: u32,
    transmission: f32,
    eta: f32,
    subsurface: f32,
    absorption: vec3<f32>,
    specular: f32,

    specular_tint: vec3<f32>,
    anisotropic: f32,

    sheen: f32,
    sheen_texture: u32,
    clearcoat: f32,
    clearcoat_texture: u32,

    clearcoat_roughness: f32,
    clearcoat_roughness_texture: u32,
    alpha_cutoff: f32,
    sheen_tint_texture: u32,

    sheen_tint: vec3<f32>,
    transmission_texture: u32,
}

struct Material {
    color: vec3<f32>,
    luminance: f32,
    metallic: f32,
    roughness: f32,
    emission: vec3<f32>,
    transmission: f32,
    eta: f32,
    subsurface: f32,
    absorption: vec3<f32>,
    specular: f32,
    specular_tint: vec3<f32>,
    anisotropic: f32,
    sheen: f32,
    sheen_tint: vec3<f32>,
    clearcoat: f32,
    clearcoat_roughness: f32,
    alpha_cutoff: f32,
}