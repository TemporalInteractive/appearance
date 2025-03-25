use appearance_camera::Camera;
use appearance_packing::{PackedNormalizedXyz10, PackedRgb9e5};
use appearance_wgpu::wgpu::{self, util::DeviceExt};
use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec3};

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct GBufferConstants {
    camera_position: Vec3,
    _padding0: u32,
    prev_camera_position: Vec3,
    _padding1: u32,
    resolution: UVec2,
    _padding2: u32,
    _padding3: u32,
}

#[repr(C)]
pub struct PackedGBufferTexel {
    position_ws: Vec3,
    depth_ws: f32,
    normal_ws: PackedNormalizedXyz10,
    albedo: PackedRgb9e5,
    _padding0: u32,
    _padding1: u32,
}

pub struct GBuffer {
    gbuffer: [wgpu::Buffer; 2],
    constants: wgpu::Buffer,
    resolution: UVec2,
    frame_idx: u32,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GBuffer {
    pub fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let gbuffer = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("appearance-path-tracer-gpu::gbuffer {}", i)),
                size: (std::mem::size_of::<PackedGBufferTexel>() as u32
                    * resolution.x
                    * resolution.y) as u64,
                mapped_at_creation: false,
                usage: wgpu::BufferUsages::STORAGE,
            })
        });

        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appearance-path-tracer-gpu::gbuffer constants"),
            contents: bytemuck::bytes_of(&GBufferConstants {
                camera_position: Vec3::ZERO,
                _padding0: 0,
                prev_camera_position: Vec3::ZERO,
                _padding1: 0,
                resolution,
                _padding2: 0,
                _padding3: 0,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        Self {
            gbuffer,
            constants,
            resolution,
            frame_idx: 0,
            bind_group_layout,
        }
    }

    pub fn write_constants(&mut self, camera: &Camera, queue: &wgpu::Queue) {
        let camera_position = camera.transform.get_translation();
        let (_, _, prev_camera_position) = camera
            .transform
            .get_prev_matrix()
            .inverse()
            .to_scale_rotation_translation();

        queue.write_buffer(
            &self.constants,
            0,
            bytemuck::bytes_of(&GBufferConstants {
                camera_position,
                _padding0: 0,
                prev_camera_position,
                _padding1: 0,
                resolution: self.resolution,
                _padding2: 0,
                _padding3: 0,
            }),
        );
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let gbuffer = &self.gbuffer[(self.frame_idx as usize) % 2];
        let prev_gbuffer = &self.gbuffer[(self.frame_idx as usize + 1) % 2];

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.constants.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: gbuffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: prev_gbuffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn end_frame(&mut self) {
        self.frame_idx += 1;
    }
}
