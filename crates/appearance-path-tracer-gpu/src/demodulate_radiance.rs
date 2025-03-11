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
    resolution: UVec2,
    remodulate: u32,
    _padding0: u32,
}

pub struct DemodulateRadiancePassParameters<'a> {
    pub resolution: UVec2,
    pub remodulate: bool,
    pub in_radiance: &'a wgpu::Buffer,
    pub out_radiance: &'a wgpu::Buffer,
    pub gbuffer: &'a GBuffer,
}

pub fn encode(
    parameters: &DemodulateRadiancePassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_shader_src!(
            "crates/appearance-path-tracer-gpu/assets/shaders/demodulate_radiance.wgsl"
        ),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("appearance-path-tracer-gpu::demodulate_radiance"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appearance-path-tracer-gpu::demodulate_radiance"),
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
        label: Some("appearance-path-tracer-gpu::demodulate_radiance constants"),
        contents: bytemuck::bytes_of(&Constants {
            resolution: parameters.resolution,
            remodulate: parameters.remodulate as u32,
            _padding0: 0,
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
                resource: parameters.in_radiance.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.out_radiance.as_entire_binding(),
            },
        ],
    });

    {
        let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("appearance-path-tracer-gpu::demodulate_radiance"),
            timestamp_writes: None,
        });
        cpass.set_pipeline(&pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        cpass.set_bind_group(1, empty_bind_group(device), &[]);
        cpass.set_bind_group(2, empty_bind_group(device), &[]);
        cpass.set_bind_group(3, empty_bind_group(device), &[]);
        cpass.set_bind_group(4, &parameters.gbuffer.bind_group(device), &[]);
        cpass.insert_debug_marker("appearance-path-tracer-gpu::demodulate_radiance");
        cpass.dispatch_workgroups(
            parameters.resolution.x.div_ceil(16),
            parameters.resolution.y.div_ceil(16),
            1,
        );
    }
}
