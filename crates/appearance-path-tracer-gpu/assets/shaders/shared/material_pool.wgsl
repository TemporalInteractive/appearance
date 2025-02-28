@include ::math

const INVALID_TEXTURE: u32 = U32_MAX;
const MAX_MATERIAL_POOL_TEXTURES: u32 = 1024u;

struct MaterialDescriptor {
    base_color_factor: vec4<f32>,
    base_color_texture: u32,

    occlusion_strength: f32,
    occlusion_texture: u32,

    metallic_factor: f32,
    roughness_factor: f32,
    metallic_roughness_texture: u32,

    ior: f32,
    transmission_factor: f32,

    emissive_factor: vec3<f32>,
    emissive_texture: u32,

    alpha_cutoff: f32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}