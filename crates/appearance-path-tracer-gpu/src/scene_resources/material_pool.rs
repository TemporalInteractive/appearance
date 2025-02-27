use std::{num::NonZeroU32, sync::Arc};

use appearance_wgpu::wgpu;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};

pub const MAX_MATERIAL_POOL_MATERIALS: usize = 1024 * 16;
pub const MAX_MATERIAL_POOL_TEXTURES: usize = 1024;

pub struct MaterialPoolAlloc {
    pub material_descriptor: MaterialDescriptor,
    pub index: u32,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct MaterialDescriptor {
    pub base_color_factor: Vec4,
    base_color_texture: u32,
    pub occlusion_strength: f32,
    occlusion_texture: u32,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    metallic_roughness_texture: u32,
    pub ior: f32,
    pub transmission_factor: f32,
    pub emissive_factor: Vec3,
    emissive_texture: u32,
}

pub struct MaterialPool {
    material_descriptor_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    empty_texture_view: wgpu::TextureView,
    texture_views: Vec<wgpu::TextureView>,

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
            Self::create_texture(device, 1, 1, wgpu::TextureFormat::Rgba8UnormSrgb);

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

            material_descriptors: Vec::new(),
            bind_group_layout,
        }
    }

    fn create_texture(
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
            label: None,
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
        let (texture, texture_view) = Self::create_texture(device, width, height, format);

        // let encoded_image = match format {
        //     TextureFormat::Bc7RgbaUnormSrgb => image_dds::dds_from_image(
        //         &model_texture.image.to_rgba8(),
        //         ImageFormat::BC7RgbaUnormSrgb,
        //         image_dds::Quality::Fast,
        //         Mipmaps::Disabled,
        //     )
        //     .unwrap(),
        //     TextureFormat::Bc7RgbaUnorm => image_dds::dds_from_image(
        //         &model_texture.image.to_rgba8(),
        //         ImageFormat::BC7RgbaUnorm,
        //         image_dds::Quality::Fast,
        //         Mipmaps::Disabled,
        //     )
        //     .unwrap(),
        //     TextureFormat::Bc6hRgbUfloat => image_dds::dds_from_imagef32(
        //         &model_texture.image.to_rgba32f(),
        //         ImageFormat::BC6hRgbUfloat,
        //         image_dds::Quality::Fast,
        //         Mipmaps::Disabled,
        //     )
        //     .unwrap(),
        //     _ => panic!("Unsupported texture format"),
        // };

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            model_texture.data(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(model_texture.width() * 4),
                rows_per_image: Some(model_texture.height()),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.texture_views.push(texture_view);
        let texture_idx = self.texture_views.len() - 1;

        // TODO: texture indices ARE needed, the appearance texture will require a uuid for quick identifying tho
        self.texture_indices
            .insert(*model_texture.uuid(), texture_idx);
        texture_idx as u32
    }

    pub fn write_material_descriptors(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.material_descriptor_buffer,
            0,
            bytemuck::cast_slice(self.material_descriptors.as_slice()),
        );
    }

    pub fn alloc(&mut self, num_vertices: u32, num_indices: u32) -> VertexPoolAlloc {
        let first_vertex = self
            .first_available_vertex(num_vertices)
            .expect("Vertex pool ran out of vertices!");
        let first_index = self
            .first_available_index(num_indices)
            .expect("Vertex pool ran out of indices!");

        let slice = VertexPoolSlice {
            first_vertex,
            num_vertices,
            first_index,
            num_indices,
            material_idx: 0,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
        };
        self.slices.push(slice);

        VertexPoolAlloc {
            slice,
            index: self.slices.len() as u32 - 1,
        }
    }

    pub fn free(_index: u32) {
        todo!()
    }

    fn first_available_vertex(&self, num_vertices: u32) -> Option<u32> {
        if self.slices.is_empty() && MAX_VERTEX_POOL_VERTICES as u32 > num_vertices {
            return Some(0);
        }

        for i in 0..self.slices.len() {
            let prev = if i > 0 {
                self.slices[i - 1].last_vertex()
            } else {
                0
            };

            let space = self.slices[i].first_vertex - prev;
            if space >= num_vertices {
                return Some(prev + num_vertices);
            }
        }

        let back = self.slices.last().unwrap().last_vertex();
        if back + num_vertices <= MAX_VERTEX_POOL_VERTICES as u32 {
            return Some(back);
        }

        None
    }

    fn first_available_index(&self, num_indices: u32) -> Option<u32> {
        if self.slices.is_empty() && MAX_VERTEX_POOL_VERTICES as u32 * 3 > num_indices {
            return Some(0);
        }

        for i in 0..self.slices.len() {
            let prev = if i > 0 {
                self.slices[i - 1].last_index()
            } else {
                0
            };

            let space = self.slices[i].first_index - prev;
            if space >= num_indices {
                return Some(prev + num_indices);
            }
        }

        let back = self.slices.last().unwrap().last_index();
        if back + num_indices <= MAX_VERTEX_POOL_VERTICES as u32 {
            return Some(back);
        }

        None
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn vertex_position_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_position_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }
}
