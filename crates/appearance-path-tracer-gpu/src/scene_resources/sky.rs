use std::sync::Arc;

use appearance_texture::Texture;
use appearance_wgpu::{
    empty_texture_view,
    wgpu::{self, util::DeviceExt},
};
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[derive(Debug, Pod, Clone, Copy, Zeroable)]
#[repr(C)]
pub struct SunInfo {
    // Normalized sun direction
    pub direction: Vec3,
    // Radius in angular radians scaled by a magnitude of 10
    pub size: f32,
    // Artistic color, is used as normalized
    pub color: Vec3,
    // Intensity factor
    pub intensity: f32,
}

pub struct Sky {
    texture_view: Option<wgpu::TextureView>,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,

    pub sun_info: SunInfo,
}

impl Default for SunInfo {
    fn default() -> Self {
        Self {
            direction: Vec3::new(-0.2, -1.0, 0.1).normalize(),
            color: Vec3::new(1.0, 1.0, 1.0),
            size: 0.5,
            intensity: 5.0, //50.0,
        }
    }
}

impl Sky {
    pub fn new(device: &wgpu::Device) -> Self {
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
                        ty: wgpu::BufferBindingType::Uniform,
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
                    count: None,
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
            texture_view: None,
            sampler,
            bind_group_layout,
            sun_info: SunInfo::default(),
        }
    }

    pub fn set_sky_texture(
        &mut self,
        texture: &Arc<Texture>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        self.texture_view = Some(
            texture
                .create_wgpu_texture(true, wgpu::TextureUsages::TEXTURE_BINDING, device, queue)
                .1,
        );
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appearance-path-tracer-gpu::sky constants"),
            contents: bytemuck::bytes_of(&self.sun_info),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let sky_texture_view = self
            .texture_view
            .as_ref()
            .unwrap_or(empty_texture_view(device));

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: constants.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(sky_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }
}
