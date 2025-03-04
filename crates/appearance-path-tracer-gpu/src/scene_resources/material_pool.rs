use std::{collections::HashMap, num::NonZeroU32, sync::Arc};

use appearance_asset_database::Asset;
use appearance_model::material::Material;
use appearance_wgpu::{empty_texture_view, wgpu};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use uuid::Uuid;

pub const MAX_MATERIAL_POOL_MATERIALS: usize = 1024 * 8;
pub const MAX_MATERIAL_POOL_TEXTURES: usize = 1024;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct MaterialDescriptor {
    pub color: Vec3,
    pub color_texture: u32,
    pub metallic: f32,
    pub roughness: f32,
    pub metallic_roughness_texture: u32,
    pub normal_scale: f32,
    pub emission: Vec3,
    pub normal_texture: u32,
    pub emission_texture: u32,
    pub transmission: f32,
    pub eta: f32,
    pub subsurface: f32,
    pub absorption: Vec3,
    pub specular: f32,
    pub specular_tint: Vec3,
    pub anisotropic: f32,
    pub sheen: f32,
    pub sheen_texture: u32,
    pub clearcoat: f32,
    pub clearcoat_texture: u32,
    pub clearcoat_roughness: f32,
    pub clearcoat_roughness_texture: u32,
    pub alpha_cutoff: f32,
    pub sheen_tint_texture: u32,
    pub sheen_tint: Vec3,
    pub transmission_texture: u32,
}

pub struct MaterialPool {
    material_descriptor_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    texture_views: Vec<wgpu::TextureView>,
    texture_indices: HashMap<Uuid, usize>,

    material_descriptors: Vec<MaterialDescriptor>,

    bind_group_layout: wgpu::BindGroupLayout,
}

impl MaterialPool {
    pub fn new(device: &wgpu::Device) -> Self {
        let material_descriptor_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::material_pool material_descriptors"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<MaterialDescriptor>() * MAX_MATERIAL_POOL_MATERIALS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: Some(NonZeroU32::new(MAX_MATERIAL_POOL_TEXTURES as u32).unwrap()),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            material_descriptor_buffer,
            sampler,
            texture_views: Vec::new(),
            texture_indices: HashMap::new(),

            material_descriptors: Vec::new(),
            bind_group_layout,
        }
    }

    fn alloc_texture(
        &mut self,
        model_texture: &Arc<appearance_texture::Texture>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        let (_texture, texture_view) = model_texture.create_wgpu_texture(
            true,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            device,
            queue,
        );

        self.texture_views.push(texture_view);
        let texture_idx = self.texture_views.len() - 1;

        self.texture_indices
            .insert(model_texture.uuid(), texture_idx);
        texture_idx as u32
    }

    pub fn material_count(&self) -> usize {
        self.material_descriptors.len()
    }

    pub fn alloc_material(
        &mut self,
        material: &Material,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        let color_texture = if let Some(texture) = &material.color_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let metallic_roughness_texture = if let Some(texture) = &material.metallic_roughness_texture
        {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let emission_texture = if let Some(texture) = &material.emission_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let normal_texture = if let Some(texture) = &material.normal_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let clearcoat_texture = if let Some(texture) = &material.clearcoat_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let clearcoat_roughness_texture =
            if let Some(texture) = &material.clearcoat_roughness_texture {
                if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                    *texture_idx as u32
                } else {
                    self.alloc_texture(texture, device, queue)
                }
            } else {
                u32::MAX
            };
        let transmission_texture = if let Some(texture) = &material.transmission_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let sheen_texture = if let Some(texture) = &material.sheen_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };
        let sheen_tint_texture = if let Some(texture) = &material.sheen_tint_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(texture, device, queue)
            }
        } else {
            u32::MAX
        };

        let material_descriptor = MaterialDescriptor {
            color: material.color,
            color_texture,
            metallic: material.metallic,
            roughness: material.roughness,
            metallic_roughness_texture,
            normal_scale: material.normal_scale,
            emission: material.emission,
            normal_texture,
            emission_texture,
            transmission: material.transmission,
            transmission_texture,
            eta: material.eta,
            subsurface: material.subsurface,
            absorption: material.absorption,
            specular: material.specular,
            specular_tint: material.specular_tint,
            anisotropic: material.anisotropic,
            sheen: material.sheen,
            sheen_tint: material.sheen_tint,
            clearcoat: material.clearcoat,
            clearcoat_texture,
            clearcoat_roughness: material.clearcoat_roughness,
            clearcoat_roughness_texture,
            alpha_cutoff: material.alpha_cutoff,
            sheen_texture,
            sheen_tint_texture,
        };

        self.material_descriptors.push(material_descriptor);
        self.material_descriptors.len() as u32 - 1
    }

    pub fn write_materials(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.material_descriptor_buffer,
            0,
            bytemuck::cast_slice(self.material_descriptors.as_slice()),
        );
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group<F>(
        &self,
        pipeline: &wgpu::ComputePipeline,
        device: &wgpu::Device,
        mut callback: F,
    ) where
        F: FnMut(&wgpu::BindGroup),
    {
        let mut entries = vec![];
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.material_descriptor_buffer.as_entire_binding(),
        });

        let mut texture_views = vec![];
        for texture in &self.texture_views {
            texture_views.push(texture);
        }
        for _ in 0..(MAX_MATERIAL_POOL_TEXTURES - self.texture_views.len()) {
            texture_views.push(empty_texture_view(device));
        }

        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureViewArray(&texture_views),
        });

        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        });

        let bind_group_layout = pipeline.get_bind_group_layout(2);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &entries,
        });

        callback(&bind_group);
    }
}
