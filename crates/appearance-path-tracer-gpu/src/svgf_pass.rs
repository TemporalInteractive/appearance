use appearance_packing::PackedRgb9e5;
use appearance_wgpu::{
    empty_bind_group, empty_bind_group_layout, include_shader_src,
    pipeline_database::PipelineDatabase,
    wgpu::{self, util::DeviceExt},
    ComputePipelineDescriptorExtensions,
};
use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec2};

use crate::gbuffer::GBuffer;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    resolution: UVec2,
    history_influence: f32,
    seed: u32,
}

pub struct SvgfPassParameters<'a> {
    pub resolution: UVec2,
    pub history_influence: f32,
    pub demodulated_radiance: &'a wgpu::Buffer,
    pub gbuffer: &'a GBuffer,
    pub velocity_texture_view: &'a wgpu::TextureView,
}

pub struct SvgfPass {
    temporal_demodulated_radiance: [wgpu::Buffer; 2],
    temporal_moments: [wgpu::Buffer; 2],
    temporal_frame_count: wgpu::Buffer,
    frame_idx: u32,
}

impl SvgfPass {
    pub fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let temporal_demodulated_radiance = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!(
                    "appearance-path-tracer-gpu::svgf_temporal temporal_demodulated_radiance {}",
                    i,
                )),
                size: (std::mem::size_of::<PackedRgb9e5>() as u32 * resolution.x * resolution.y)
                    as u64,
                mapped_at_creation: false,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            })
        });

        let temporal_moments = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!(
                    "appearance-path-tracer-gpu::svgf_temporal temporal_moments {}",
                    i,
                )),
                size: (std::mem::size_of::<Vec2>() as u32 * resolution.x * resolution.y) as u64,
                mapped_at_creation: false,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            })
        });

        let temporal_frame_count = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::svgf_temporal temporal_frame_count"),
            size: (std::mem::size_of::<u32>() as u32 * resolution.x * resolution.y) as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });

        Self {
            temporal_demodulated_radiance,
            temporal_moments,
            temporal_frame_count,
            frame_idx: 0,
        }
    }

    pub fn encode(
        &self,
        parameters: &SvgfPassParameters,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        pipeline_database: &mut PipelineDatabase,
    ) {
        let shader = pipeline_database.shader_from_src(
            device,
            include_shader_src!(
                "crates/appearance-path-tracer-gpu/assets/shaders/svgf_temporal.wgsl"
            ),
        );
        let pipeline = pipeline_database.compute_pipeline(
            device,
            wgpu::ComputePipelineDescriptor {
                label: Some("appearance-path-tracer-gpu::svgf_temporal"),
                ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
            },
            || {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("appearance-path-tracer-gpu::svgf_temporal"),
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
                                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                                        has_dynamic_offset: false,
                                        min_binding_size: None,
                                    },
                                    count: None,
                                },
                                wgpu::BindGroupLayoutEntry {
                                    binding: 3,
                                    visibility: wgpu::ShaderStages::COMPUTE,
                                    ty: wgpu::BindingType::Buffer {
                                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                                        has_dynamic_offset: false,
                                        min_binding_size: None,
                                    },
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
                                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                                        has_dynamic_offset: false,
                                        min_binding_size: None,
                                    },
                                    count: None,
                                },
                                wgpu::BindGroupLayoutEntry {
                                    binding: 6,
                                    visibility: wgpu::ShaderStages::COMPUTE,
                                    ty: wgpu::BindingType::Buffer {
                                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                                        has_dynamic_offset: false,
                                        min_binding_size: None,
                                    },
                                    count: None,
                                },
                                wgpu::BindGroupLayoutEntry {
                                    binding: 7,
                                    visibility: wgpu::ShaderStages::COMPUTE,
                                    ty: wgpu::BindingType::StorageTexture {
                                        access: wgpu::StorageTextureAccess::ReadOnly,
                                        format: wgpu::TextureFormat::Rgba32Float,
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
            label: Some("appearance-path-tracer-gpu::svgf_temporal constants"),
            contents: bytemuck::bytes_of(&Constants {
                resolution: parameters.resolution,
                history_influence: parameters.history_influence,
                seed: self.frame_idx,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let in_temporal_demodulated_radiance =
            &self.temporal_demodulated_radiance[(self.frame_idx as usize) % 2];
        let out_temporal_demodulated_radiance =
            &self.temporal_demodulated_radiance[(self.frame_idx as usize + 1) % 2];
        let in_temporal_moments = &self.temporal_moments[(self.frame_idx as usize) % 2];
        let out_temporal_moments = &self.temporal_moments[(self.frame_idx as usize + 1) % 2];

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
                    resource: parameters.demodulated_radiance.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: in_temporal_demodulated_radiance.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: out_temporal_demodulated_radiance.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: in_temporal_moments.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: out_temporal_moments.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.temporal_frame_count.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(parameters.velocity_texture_view),
                },
            ],
        });

        {
            let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("appearance-path-tracer-gpu::svgf_temporal"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&pipeline);
            cpass.set_bind_group(0, &bind_group, &[]);
            cpass.set_bind_group(1, empty_bind_group(device), &[]);
            cpass.set_bind_group(2, empty_bind_group(device), &[]);
            cpass.set_bind_group(3, empty_bind_group(device), &[]);
            cpass.set_bind_group(4, &parameters.gbuffer.bind_group(device), &[]);
            cpass.insert_debug_marker("appearance-path-tracer-gpu::svgf_temporal");
            cpass.dispatch_workgroups(
                parameters.resolution.x.div_ceil(16),
                parameters.resolution.y.div_ceil(16),
                1,
            );
        }

        command_encoder.copy_buffer_to_buffer(
            out_temporal_demodulated_radiance,
            0,
            parameters.demodulated_radiance,
            0,
            out_temporal_demodulated_radiance.size(),
        );
    }

    pub fn end_frame(&mut self) {
        self.frame_idx += 1;
    }
}
