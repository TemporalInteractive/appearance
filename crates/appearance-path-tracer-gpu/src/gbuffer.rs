use appearance_camera::{frustum::FrustumSide, Camera};
use appearance_packing::{PackedNormalizedXyz10, PackedRgb9e5};
use appearance_wgpu::wgpu::{self, util::DeviceExt};
use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec3, Vec4};

#[derive(Pod, Clone, Copy, Zeroable, Default)]
#[repr(C)]
struct Frustum {
    left: Vec4,
    right: Vec4,
    top: Vec4,
    bottom: Vec4,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct GBufferConstants {
    prev_camera_frustum: Frustum,
    resolution: UVec2,
    _padding0: u32,
    _padding1: u32,
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
    resolution: UVec2,
    frame_idx: u32,
    prev_camera_frustum: Frustum,
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
            resolution,
            frame_idx: 0,
            prev_camera_frustum: Frustum::default(),
            bind_group_layout,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appearance-path-tracer-gpu::gbuffer constants"),
            contents: bytemuck::bytes_of(&GBufferConstants {
                prev_camera_frustum: self.prev_camera_frustum,
                resolution: self.resolution,
                _padding0: 0,
                _padding1: 0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let gbuffer = &self.gbuffer[(self.frame_idx as usize) % 2];
        let prev_gbuffer = &self.gbuffer[(self.frame_idx as usize + 1) % 2];

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
                    resource: gbuffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: prev_gbuffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn end_frame(&mut self, camera: &Camera) {
        self.frame_idx += 1;

        let prev_camera_frustum = camera.build_prev_frustum();
        self.prev_camera_frustum = Frustum {
            left: prev_camera_frustum.get_plane(FrustumSide::Left).into(),
            right: prev_camera_frustum.get_plane(FrustumSide::Right).into(),
            top: prev_camera_frustum.get_plane(FrustumSide::Top).into(),
            bottom: prev_camera_frustum.get_plane(FrustumSide::Bottom).into(),
        };
    }
}
