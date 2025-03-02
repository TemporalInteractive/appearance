use std::sync::Arc;

use appearance_texture::Texture;
use glam::Vec3;

#[derive(Debug, Clone)]
pub struct Material {
    pub index: Option<usize>,

    pub color: Vec3,
    pub color_texture: Option<Arc<Texture>>,
    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: Option<Arc<Texture>>,
    pub normal_scale: f32,
    pub normal_texture: Option<Arc<Texture>>,
    pub emission: Vec3,
    pub emission_texture: Option<Arc<Texture>>,

    pub absorption: Vec3,
    pub transmission: f32,
    pub eta: f32,

    pub subsurface: f32,
    pub specular: f32,
    pub specular_tint: f32,
    pub anisotropic: f32,
    pub sheen: f32,
    pub sheen_tint: f32,
    pub clearcoat: f32,
    pub clearcoat_gloss: f32,

    pub is_opaque: bool,
    pub alpha_cutoff: f32,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            index: None,
            color: Vec3::new(1.0, 1.0, 1.0),
            color_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            metallic_roughness_texture: None,
            normal_scale: 1.0,
            normal_texture: None,
            emission: Vec3::ZERO,
            emission_texture: None,

            absorption: Vec3::ZERO,
            transmission: 0.0,
            eta: 1.0 / 1.45,

            subsurface: 0.0,
            specular: 0.0,
            specular_tint: 0.0,
            anisotropic: 0.0,
            sheen: 0.0,
            sheen_tint: 0.0,
            clearcoat: 0.0,
            clearcoat_gloss: 0.0,

            is_opaque: true,
            alpha_cutoff: 0.0,
        }
    }
}

// #[derive(Debug, Clone)]
// pub struct Material {
//     pub index: Option<usize>,

//     pub base_color_factor: Vec4,
//     pub base_color_texture: Option<Arc<Texture>>,

//     pub normal_scale: f32,
//     pub normal_texture: Option<Arc<Texture>>,

//     pub metallic_factor: f32,
//     pub roughness_factor: f32,
//     pub metallic_roughness_texture: Option<Arc<Texture>>,

//     pub occlusion_strength: f32,
//     pub occlusion_texture: Option<Arc<Texture>>,

//     pub emissive_factor: Vec3,
//     pub emissive_texture: Option<Arc<Texture>>,

//     pub ior: f32,
//     pub transmission_factor: f32,

//     pub opaque: bool,
//     pub alpha_cutoff: f32,
// }

// impl Default for Material {
//     fn default() -> Self {
//         Material {
//             index: None,
//             base_color_factor: Vec4::new(1.0, 1.0, 1.0, 1.0),
//             base_color_texture: None,
//             normal_scale: 1.0,
//             normal_texture: None,
//             metallic_factor: 0.0,
//             roughness_factor: 1.0,
//             metallic_roughness_texture: None,
//             occlusion_strength: 1.0,
//             occlusion_texture: None,
//             emissive_factor: Vec3::ZERO,
//             emissive_texture: None,
//             ior: 1.5,
//             transmission_factor: 0.0,
//             opaque: true,
//             alpha_cutoff: 0.0,
//         }
//     }
// }
