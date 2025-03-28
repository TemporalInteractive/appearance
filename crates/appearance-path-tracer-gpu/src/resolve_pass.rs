use appearance_wgpu::{
    empty_bind_group, empty_bind_group_layout, include_shader_src,
    pipeline_database::PipelineDatabase,
    wgpu::{self, util::DeviceExt},
    ComputePipelineDescriptorExtensions,
};
use bytemuck::{Pod, Zeroable};
use glam::UVec2;

use crate::gbuffer::GBuffer;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    width: u32,
    height: u32,
    sample_count: u32,
    accum_frame_count: u32,
}

pub struct ResolvePassParameters<'a> {
    pub resolution: UVec2,
    pub sample_count: u32,
    pub accum_frame_count: u32,
    pub radiance: &'a wgpu::Buffer,
    pub accum_radiance: &'a wgpu::Buffer,
    pub gbuffer: &'a GBuffer,
    pub target_view: &'a wgpu::TextureView,
}

pub fn encode(
    parameters: &ResolvePassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_shader_src!("crates/appearance-path-tracer-gpu/assets/shaders/resolve.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("appearance-path-tracer-gpu::resolve"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appearance-path-tracer-gpu::resolve"),
                bind_group_layouts: &[
                    &device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 2,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 3,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::StorageTexture {
                                    access: wgpu::StorageTextureAccess::ReadWrite,
                                    format: wgpu::TextureFormat::Rgba8Unorm,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                },
                                count: None,
                            },
                        ],
                    }),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    empty_bind_group_layout(device),
                    parameters.gbuffer.bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("appearance-path-tracer-gpu::resolve constants"),
        contents: bytemuck::bytes_of(&Constants {
            width: parameters.resolution.x,
            height: parameters.resolution.y,
            sample_count: parameters.sample_count,
            accum_frame_count: parameters.accum_frame_count,
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
                resource: parameters.radiance.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.accum_radiance.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(parameters.target_view),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("appearance-path-tracer-gpu::resolve"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(1, empty_bind_group(device), &[]);
        cpass.set_bind_group(2, empty_bind_group(device), &[]);
        cpass.set_bind_group(3, empty_bind_group(device), &[]);
        cpass.set_bind_group(4, &parameters.gbuffer.bind_group(device), &[]);
        cpass.insert_debug_marker("appearance-path-tracer-gpu::resolve");
        cpass.dispatch_workgroups(
            parameters.resolution.x.div_ceil(16),
            parameters.resolution.y.div_ceil(16),
            1,
        );
    }
}
