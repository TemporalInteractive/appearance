use std::{collections::HashMap, num::NonZeroU32, sync::Arc};

use appearance_asset_database::Asset;
use appearance_model::material::Material;
use appearance_wgpu::wgpu;
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
    _padding0: u32,
    pub clearcoat: f32,
    pub clearcoat_texture: u32,
    pub clearcoat_roughness: f32,
    pub clearcoat_roughness_texture: u32,
    pub alpha_cutoff: f32,
    _padding1: u32,
    pub sheen_tint: Vec3,
    _padding2: u32,
}

pub struct MaterialPool {
    material_descriptor_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    empty_texture_view: wgpu::TextureView,
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

        let (_, empty_texture_view) =
            Self::create_texture("Empty", device, 1, 1, wgpu::TextureFormat::Rgba8UnormSrgb);

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
            empty_texture_view,
            texture_views: Vec::new(),
            texture_indices: HashMap::new(),

            material_descriptors: Vec::new(),
            bind_group_layout,
        }
    }

    fn create_texture(
        name: &str,
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some(name),
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });
        (texture, texture_view)
    }

    fn alloc_texture(
        &mut self,
        model_texture: &Arc<appearance_texture::Texture>,
        format: wgpu::TextureFormat,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> u32 {
        let width = model_texture.width();
        let height = model_texture.height();
        let (texture, texture_view) =
            Self::create_texture(model_texture.name(), device, width, height, format);

        let texture_data = match format {
            wgpu::TextureFormat::Bc4RUnorm => {
                let surface = intel_tex_2::RSurface {
                    width,
                    height,
                    stride: width * model_texture.format().num_channels() as u32,
                    data: model_texture.data(),
                };

                intel_tex_2::bc4::compress_blocks(&surface)
            }
            wgpu::TextureFormat::Bc5RgUnorm => {
                let surface = intel_tex_2::RgSurface {
                    width,
                    height,
                    stride: width * model_texture.format().num_channels() as u32,
                    data: model_texture.data(),
                };

                intel_tex_2::bc5::compress_blocks(&surface)
            }
            wgpu::TextureFormat::Bc7RgbaUnorm => {
                let surface = intel_tex_2::RgbaSurface {
                    width,
                    height,
                    stride: width * model_texture.format().num_channels() as u32,
                    data: model_texture.data(),
                };

                intel_tex_2::bc7::compress_blocks(
                    &intel_tex_2::bc7::alpha_ultra_fast_settings(),
                    &surface,
                )
            }
            _ => panic!("Unsupported texture format."),
        };

        let block_size = match format {
            wgpu::TextureFormat::Bc1RgbaUnorm
            | wgpu::TextureFormat::Bc4RUnorm
            | wgpu::TextureFormat::Bc4RSnorm => 8,
            wgpu::TextureFormat::Bc2RgbaUnorm
            | wgpu::TextureFormat::Bc3RgbaUnorm
            | wgpu::TextureFormat::Bc5RgUnorm
            | wgpu::TextureFormat::Bc5RgSnorm
            | wgpu::TextureFormat::Bc7RgbaUnorm => 16,
            _ => panic!("Unsupported texture format."),
        };
        let bytes_per_row = ((width + 3) / 4) * block_size;

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &texture_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
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
                self.alloc_texture(
                    texture,
                    texture.format().to_wgpu_compressed(),
                    device,
                    queue,
                )
            }
        } else {
            u32::MAX
        };
        let metallic_roughness_texture = if let Some(texture) = &material.metallic_roughness_texture
        {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(
                    texture,
                    texture.format().to_wgpu_compressed(),
                    device,
                    queue,
                )
            }
        } else {
            u32::MAX
        };
        let emission_texture = if let Some(texture) = &material.emission_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(
                    texture,
                    texture.format().to_wgpu_compressed(),
                    device,
                    queue,
                )
            }
        } else {
            u32::MAX
        };
        let normal_texture = if let Some(texture) = &material.normal_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(
                    texture,
                    texture.format().to_wgpu_compressed(),
                    device,
                    queue,
                )
            }
        } else {
            u32::MAX
        };
        let clearcoat_texture = if let Some(texture) = &material.clearcoat_texture {
            if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                *texture_idx as u32
            } else {
                self.alloc_texture(
                    texture,
                    texture.format().to_wgpu_compressed(),
                    device,
                    queue,
                )
            }
        } else {
            u32::MAX
        };
        let clearcoat_roughness_texture =
            if let Some(texture) = &material.clearcoat_roughness_texture {
                if let Some(texture_idx) = self.texture_indices.get(&texture.uuid()) {
                    *texture_idx as u32
                } else {
                    self.alloc_texture(
                        texture,
                        texture.format().to_wgpu_compressed(),
                        device,
                        queue,
                    )
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
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
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
            texture_views.push(&self.empty_texture_view);
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
