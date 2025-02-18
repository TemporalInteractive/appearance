use std::sync::Arc;

use appearance_texture::Texture;
use glam::{Vec3, Vec4};

#[derive(Debug, Clone)]
pub struct Material {
    pub index: Option<usize>,

    pub base_color_factor: Vec4,
    pub base_color_texture: Option<Arc<Texture>>,

    pub normal_scale: f32,
    pub normal_texture: Option<Arc<Texture>>,

    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub metallic_roughness_texture: Option<Arc<Texture>>,

    pub occlusion_strength: f32,
    pub occlusion_texture: Option<Arc<Texture>>,

    pub emissive_factor: Vec3,
    pub emissive_texture: Option<Arc<Texture>>,

    pub ior: f32,
    pub transmission_factor: f32,
}

impl Default for Material {
    fn default() -> Self {
        Material {
            index: None,
            base_color_factor: Vec4::new(1.0, 1.0, 1.0, 1.0),
            base_color_texture: None,
            normal_scale: 1.0,
            normal_texture: None,
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
            occlusion_strength: 1.0,
            occlusion_texture: None,
            emissive_factor: Vec3::ZERO,
            emissive_texture: None,
            ior: 1.5,
            transmission_factor: 0.0,
        }
    }
}
