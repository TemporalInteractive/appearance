use appearance_wgpu::{
    include_shader_src,
    pipeline_database::PipelineDatabase,
    wgpu::{self, util::DeviceExt},
    ComputePipelineDescriptorExtensions,
};
use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec2, Vec3};

use crate::{gbuffer::GBuffer, scene_resources::SceneResources};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct TemporalConstants {
    resolution: UVec2,
    ray_count: u32,
    spatial_pass_count: u32,
    unbiased: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct SpatialConstants {
    resolution: UVec2,
    spatial_pass_count: u32,
    spatial_pass_idx: u32,
    pixel_radius: f32,
    seed: u32,
    spatial_idx: u32,
    unbiased: u32,
}

#[repr(C)]
pub struct PackedLightSample {
    point: Vec3,
    emission: u32,
    triangle_area: f32,
    triangle_normal: u32,
    _padding0: u32,
    _padding1: u32,
}

#[repr(C)]
pub struct LightSampleCtx {
    hit_tex_coord: Vec2,
    hit_material_idx: u32,
    throughput: u32,
    front_facing_shading_normal_ws: u32,
    front_facing_clearcoat_normal_ws: u32,
}

#[repr(C)]
pub struct PackedDiReservoir {
    sample_count: f32,
    contribution_weight: f32,
    weight_sum: f32,
    selected_phat: f32,
    sample: PackedLightSample,
}

pub struct RestirDiPassParameters<'a> {
    pub resolution: UVec2,
    pub seed: u32,
    pub spatial_pass_count: u32,
    pub spatial_pixel_radius: f32,
    pub unbiased: bool,
    pub rays: &'a wgpu::Buffer,
    pub payloads: &'a wgpu::Buffer,
    pub light_sample_reservoirs: &'a wgpu::Buffer,
    pub light_sample_ctxs: &'a wgpu::Buffer,
    pub gbuffer: &'a GBuffer,
    pub scene_resources: &'a SceneResources,
}

pub struct RestirDiPass {
    prev_reservoirs: wgpu::Buffer,
    intermediate_reservoirs: wgpu::Buffer,
}

impl RestirDiPass {
    pub fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let prev_reservoirs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::restir_di_pass prev_reservoirs"),
            size: (std::mem::size_of::<PackedDiReservoir>() as u32 * resolution.x * resolution.y)
                as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let intermediate_reservoirs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::restir_di_pass intermediate_reservoirs"),
            size: (std::mem::size_of::<PackedDiReservoir>() as u32 * resolution.x * resolution.y)
                as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });

        Self {
            prev_reservoirs,
            intermediate_reservoirs,
        }
    }

    pub fn encode(
        &self,
        parameters: &RestirDiPassParameters,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        pipeline_database: &mut PipelineDatabase,
    ) {
        self.encode_temporal(parameters, device, command_encoder, pipeline_database);
        self.encode_spatial(parameters, device, command_encoder, pipeline_database);
    }

    fn encode_temporal(
        &self,
        parameters: &RestirDiPassParameters,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        pipeline_database: &mut PipelineDatabase,
    ) {
        let shader = pipeline_database.shader_from_src(
            device,
            include_shader_src!(
                "crates/appearance-path-tracer-gpu/assets/shaders/restir_di_temporal.wgsl"
            ),
        );
        let pipeline = pipeline_database.compute_pipeline(
            device,
            wgpu::ComputePipelineDescriptor {
                label: Some("appearance-path-tracer-gpu::restir_di_temporal"),
                ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
            },
            || {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("appearance-path-tracer-gpu::restir_di_temporal"),
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
                                    ty: wgpu::BindingType::AccelerationStructure,
                                    count: None,
                                },
                                wgpu::BindGroupLayoutEntry {
                                    binding: 4,
                                    visibility: wgpu::ShaderStages::COMPUTE,
                                    ty: wgpu::BindingType::Buffer {
                                        ty: wgpu::BufferBindingType::Storage { read_only: false },
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
                        parameters.gbuffer.bind_group_layout(),
                    ],
                    push_constant_ranges: &[],
                })
            },
        );

        let ray_count = parameters.resolution.x * parameters.resolution.y;
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appearance-path-tracer-gpu::restir_di_temporal constants"),
            contents: bytemuck::bytes_of(&TemporalConstants {
                resolution: parameters.resolution,
                ray_count,
                spatial_pass_count: parameters.spatial_pass_count,
                unbiased: parameters.unbiased as u32,
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
                    resource: parameters.light_sample_reservoirs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.prev_reservoirs.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: parameters.light_sample_ctxs.as_entire_binding(),
                },
            ],
        });

        parameters.scene_resources.material_pool().bind_group(
            &pipeline,
            device,
            |material_pool_bind_group| {
                let mut cpass = command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("appearance-path-tracer-gpu::restir_di_temporal"),
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
                cpass.set_bind_group(4, &parameters.gbuffer.bind_group(device), &[]);
                cpass.insert_debug_marker("appearance-path-tracer-gpu::restir_di_temporal");
                cpass.dispatch_workgroups(ray_count.div_ceil(128), 1, 1);
            },
        );
    }

    fn encode_spatial(
        &self,
        parameters: &RestirDiPassParameters,
        device: &wgpu::Device,
        command_encoder: &mut wgpu::CommandEncoder,
        pipeline_database: &mut PipelineDatabase,
    ) {
        let shader = pipeline_database.shader_from_src(
            device,
            include_shader_src!(
                "crates/appearance-path-tracer-gpu/assets/shaders/restir_di_spatial.wgsl"
            ),
        );
        let pipeline = pipeline_database.compute_pipeline(
            device,
            wgpu::ComputePipelineDescriptor {
                label: Some("appearance-path-tracer-gpu::restir_di_spatial"),
                ..wgpu::ComputePipelineDescriptor::partial_default(&shader)
            },
            || {
                device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("appearance-path-tracer-gpu::restir_di_spatial"),
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
                        parameters.gbuffer.bind_group_layout(),
                    ],
                    push_constant_ranges: &[],
                })
            },
        );

        for i in 0..parameters.spatial_pass_count {
            let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("appearance-path-tracer-gpu::restir_di_spatial constants"),
                contents: bytemuck::bytes_of(&SpatialConstants {
                    resolution: parameters.resolution,
                    spatial_pass_count: parameters.spatial_pass_count,
                    spatial_pass_idx: i,
                    pixel_radius: parameters.spatial_pixel_radius,
                    seed: parameters.seed,
                    spatial_idx: i,
                    unbiased: parameters.unbiased as u32,
                }),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let in_reservoir_buffer = if i % 2 == 0 {
                parameters.light_sample_reservoirs
            } else {
                &self.intermediate_reservoirs
            };
            let out_reservoir_buffer = if i % 2 != 0 {
                parameters.light_sample_reservoirs
            } else {
                &self.intermediate_reservoirs
            };

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
                        resource: in_reservoir_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: out_reservoir_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 6,
                        resource: self.prev_reservoirs.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 7,
                        resource: parameters.light_sample_ctxs.as_entire_binding(),
                    },
                ],
            });

            parameters.scene_resources.material_pool().bind_group(
                &pipeline,
                device,
                |material_pool_bind_group| {
                    let mut cpass =
                        command_encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("appearance-path-tracer-gpu::restir_di_spatial"),
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
                    cpass.set_bind_group(
                        3,
                        &parameters.scene_resources.sky().bind_group(device),
                        &[],
                    );
                    cpass.set_bind_group(4, &parameters.gbuffer.bind_group(device), &[]);
                    cpass.insert_debug_marker("appearance-path-tracer-gpu::restir_di_spatial");
                    cpass.dispatch_workgroups(
                        parameters.resolution.x.div_ceil(16),
                        parameters.resolution.y.div_ceil(16),
                        1,
                    );
                },
            );
        }

        if parameters.spatial_pass_count % 2 != 0 {
            command_encoder.copy_buffer_to_buffer(
                &self.intermediate_reservoirs,
                0,
                parameters.light_sample_reservoirs,
                0,
                self.intermediate_reservoirs.size(),
            );
        }
    }
}
