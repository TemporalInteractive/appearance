use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::{
    include_shader_src, pipeline_database::PipelineDatabase, ComputePipelineDescriptorExtensions,
};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    width: u32,
    height: u32,
    _padding0: u32,
    _padding1: u32,
}

pub fn encode(
    size: wgpu::Extent3d,
    src_view: &wgpu::TextureView,
    dst_buffer: &wgpu::Buffer,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    appearance_profiling::profile_function!();

    let shader = pipeline_database.shader_from_src(
        device,
        include_shader_src!("crates/appearance-wgpu/assets/shaders/texture_to_buffer.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("appearance-wgpu::texture_to_buffer"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("appearance-wgpu::texture_to_buffer constants"),
        contents: bytemuck::bytes_of(&Constants {
            width: size.width,
            height: size.height,
            _padding0: 0,
            _padding1: 0,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: constants.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(src_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: dst_buffer.as_entire_binding(),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("appearance-wgpu::texture_to_buffer"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.insert_debug_marker("appearance-wgpu::texture_to_buffer");
        cpass.dispatch_workgroups(size.width.div_ceil(16), size.height.div_ceil(16), 1);
    }
}
