use appearance_wgpu::{
    include_shader_src,
    pipeline_database::PipelineDatabase,
    wgpu::{self, util::DeviceExt},
    ComputePipelineDescriptorExtensions,
};
use bytemuck::{Pod, Zeroable};

use crate::scene_resources::SceneResources;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    ray_count: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

pub struct ApplyGiPassParameters<'a> {
    pub ray_count: u32,
    pub rays: &'a wgpu::Buffer,
    pub payloads: &'a wgpu::Buffer,
    pub gi_reservoirs: &'a wgpu::Buffer,
    pub light_sample_ctxs: &'a wgpu::Buffer,
    pub scene_resources: &'a SceneResources,
}

pub fn encode(
    parameters: &ApplyGiPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let shader = pipeline_database.shader_from_src(
        device,
        include_shader_src!("crates/appearance-path-tracer-gpu/assets/shaders/apply_gi.wgsl"),
    );
    let pipeline = pipeline_database.compute_pipeline(
        device,
        wgpu::ComputePipelineDescriptor {
            label: Some("appearance-path-tracer-gpu::apply_gi"),
            ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appearance-path-tracer-gpu::apply_gi"),
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
                                    ty: wgpu::BufferBindingType::Storage { read_only: false },
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
                                ty: wgpu::BindingType::AccelerationStructure,
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 4,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                            wgpu::BindGroupLayoutEntry {
                                binding: 5,
                                visibility: wgpu::ShaderStages::COMPUTE,
                                ty: wgpu::BindingType::Buffer {
                                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                                    has_dynamic_offset: false,
                                    min_binding_size: None,
                                },
                                count: None,
                            },
                        ],
                    }),
                    parameters.scene_resources.vertex_pool().bind_group_layout(),
                    parameters
                        .scene_resources
                        .material_pool()
                        .bind_group_layout(),
                    parameters.scene_resources.sky().bind_group_layout(),
                ],
                push_constant_ranges: &[],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("appearance-path-tracer-gpu::apply_gi constants"),
        contents: bytemuck::bytes_of(&Constants {
            ray_count: parameters.ray_count,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
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
                resource: parameters.rays.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: parameters.payloads.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::AccelerationStructure(
                    parameters.scene_resources.tlas(),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: parameters.gi_reservoirs.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: parameters.light_sample_ctxs.as_entire_binding(),
            },
        ],
    });

    parameters.scene_resources.material_pool().bind_group(
        &pipeline,
        device,
        |material_pool_bind_group| {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("appearance-path-tracer-gpu::apply_gi"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.set_bind_group(
                1,
                &parameters.scene_resources.vertex_pool().bind_group(device),
                &[],
            );
            cpass.set_bind_group(2, material_pool_bind_group, &[]);
            cpass.set_bind_group(3, &parameters.scene_resources.sky().bind_group(device), &[]);
            cpass.insert_debug_marker("appearance-path-tracer-gpu::apply_gi");
            cpass.dispatch_workgroups(parameters.ray_count.div_ceil(128), 1, 1);
        },
    );
}
