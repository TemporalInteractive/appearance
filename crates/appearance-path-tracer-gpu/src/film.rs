use appearance_wgpu::{readback_buffer, wgpu};
use glam::UVec2;

pub struct Film {
    render_target: wgpu::Texture,
    render_target_view: wgpu::TextureView,
    render_target_readback_buffer: wgpu::Buffer,
}

impl Film {
    pub fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: resolution.x,
                height: resolution.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            label: None,
            view_formats: &[],
        });
        let render_target_view = render_target.create_view(&wgpu::TextureViewDescriptor::default());

        let render_target_readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu render_target_readback_buffer"),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            size: (resolution.x * resolution.y * 4) as u64,
            mapped_at_creation: false,
        });

        Self {
            render_target,
            render_target_view,
            render_target_readback_buffer,
        }
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.render_target_view
    }

    pub fn prepare_pixel_readback(&self, command_encoder: &mut wgpu::CommandEncoder) {
        command_encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.render_target_readback_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.render_target.width() * 4),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.render_target.width(),
                height: self.render_target.height(),
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn readback_pixels(&self, device: &wgpu::Device) -> Vec<u8> {
        readback_buffer(&self.render_target_readback_buffer, device)
    }
}
