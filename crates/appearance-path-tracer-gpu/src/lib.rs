use appearance_wgpu::{
    helper_passes::texture_to_buffer_pass,
    include_shader_src,
    pipeline_database::PipelineDatabase,
    readback_buffer,
    wgpu::{self, util::DeviceExt, Extent3d},
    ComputePipelineDescriptorExtensions, Context,
};
use appearance_world::visible_world_action::VisibleWorldActionType;
use bytemuck::{Pod, Zeroable};
use glam::UVec2;

pub struct PathTracerGpu {
    resolution: UVec2,
    local_resolution: UVec2,
    render_target: wgpu::Texture,
    render_target_view: wgpu::TextureView,
    render_target_readback_buffer: wgpu::Buffer,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct RaygenConstants {
    width: u32,
    height: u32,
    _padding0: u32,
    _padding1: u32,
}

impl PathTracerGpu {
    pub fn new(ctx: &Context) -> Self {
        let resolution = UVec2::new(1920, 1080);

        let render_target = ctx.device.create_texture(&wgpu::TextureDescriptor {
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

        let render_target_readback_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu render_target_readback_buffer"),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::STORAGE,
            size: (resolution.x * resolution.y * 4) as u64,
            mapped_at_creation: false,
        });

        Self {
            resolution,
            local_resolution: resolution,
            render_target,
            render_target_view,
            render_target_readback_buffer,
        }
    }

    pub fn handle_visible_world_action(&mut self, _action: &VisibleWorldActionType) {}

    fn resize(&mut self, resolution: UVec2, start_row: u32, end_row: u32, ctx: &Context) {
        let local_resolution = UVec2::new(resolution.x, end_row - start_row);

        if self.resolution != resolution || self.local_resolution != local_resolution {
            self.resolution = resolution;
            self.local_resolution = local_resolution;

            self.render_target = ctx.device.create_texture(&wgpu::TextureDescriptor {
                size: wgpu::Extent3d {
                    width: local_resolution.x,
                    height: local_resolution.y,
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
            self.render_target_view = self
                .render_target
                .create_view(&wgpu::TextureViewDescriptor::default());

            self.render_target_readback_buffer =
                ctx.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("appearance-path-tracer-gpu render_target_readback_buffer"),
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::STORAGE,
                    size: (local_resolution.x * local_resolution.y * 4) as u64,
                    mapped_at_creation: false,
                });
        }
    }

    pub fn render<F: FnMut(&[u8])>(
        &mut self,
        resolution: UVec2,
        start_row: u32,
        end_row: u32,
        mut result_callback: F,
        ctx: &Context,
        pipeline_database: &mut PipelineDatabase,
    ) {
        self.resize(resolution, start_row, end_row, ctx);

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let shader = pipeline_database.shader_from_src(
            &ctx.device,
            include_shader_src!("crates/appearance-path-tracer-gpu/assets/shaders/raygen.wgsl"),
        );
        let pipeline = pipeline_database.compute_pipeline(
            &ctx.device,
            wgpu::ComputePipelineDescriptor {
                label: Some("appearance-path-tracer-gpu::raygen"),
                ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
            },
        );

        let constants = ctx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("appearance-path-tracer-gpu::raygen constants"),
                contents: bytemuck::bytes_of(&RaygenConstants {
                    width: self.local_resolution.x,
                    height: self.local_resolution.y,
                    _padding0: 0,
                    _padding1: 0,
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: constants.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.render_target_view),
                },
            ],
        });

        {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("appearance-path-tracer-gpu::raygen"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.insert_debug_marker("appearance-path-tracer-gpu::raygen");
            cpass.dispatch_workgroups(
                self.local_resolution.x.div_ceil(16),
                self.local_resolution.y.div_ceil(16),
                1,
            );
        }

        ctx.queue.submit(Some(command_encoder.finish()));
        ctx.device.poll(wgpu::MaintainBase::Wait);

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // assert!((self.local_resolution.x * 4) % 256 == 0);
        // command_encoder.copy_texture_to_buffer(
        //     wgpu::TexelCopyTextureInfo {
        //         texture: &self.render_target,
        //         mip_level: 0,
        //         origin: wgpu::Origin3d::ZERO,
        //         aspect: wgpu::TextureAspect::All,
        //     },
        //     wgpu::TexelCopyBufferInfo {
        //         buffer: &self.render_target_readback_buffer,
        //         layout: wgpu::TexelCopyBufferLayout {
        //             offset: 0,
        //             bytes_per_row: Some(self.local_resolution.x * 4),
        //             rows_per_image: None,
        //         },
        //     },
        //     wgpu::Extent3d {
        //         width: self.local_resolution.x,
        //         height: self.local_resolution.y,
        //         depth_or_array_layers: 1,
        //     },
        // );

        texture_to_buffer_pass::encode(
            Extent3d {
                width: self.local_resolution.x,
                height: self.local_resolution.y,
                depth_or_array_layers: 1,
            },
            &self.render_target_view,
            &self.render_target_readback_buffer,
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        ctx.queue.submit(Some(command_encoder.finish()));
        ctx.device.poll(wgpu::MaintainBase::Wait);

        let pixels = readback_buffer(&self.render_target_readback_buffer, &ctx.device);

        result_callback(&pixels);
    }
}
