use appearance_model::mesh::PackedVertex;
use appearance_wgpu::{
    include_shader_src,
    pipeline_database::PipelineDatabase,
    wgpu::{self, util::DeviceExt},
};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

use crate::scene_resources::SceneResources;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct Constants {
    view_proj: Mat4,
}

pub struct GbufferPassParameters<'a> {
    pub view_proj: Mat4,
    pub scene_resources: &'a SceneResources,
    pub gbuffer_view: &'a wgpu::TextureView,
    pub depth_texture: &'a wgpu::Texture,
}

pub fn encode(
    parameters: &GbufferPassParameters,
    device: &wgpu::Device,
    command_encoder: &mut wgpu::CommandEncoder,
    pipeline_database: &mut PipelineDatabase,
) {
    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<PackedVertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                // Position
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                // Normal
                format: wgpu::VertexFormat::Uint32,
                offset: 3 * std::mem::size_of::<f32>() as u64,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                // Texcoord
                format: wgpu::VertexFormat::Float32x2,
                offset: 4 * std::mem::size_of::<f32>() as u64,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                // Tangent
                format: wgpu::VertexFormat::Uint32,
                offset: 6 * std::mem::size_of::<f32>() as u64,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                // Tangent handiness
                format: wgpu::VertexFormat::Float32,
                offset: 7 * std::mem::size_of::<f32>() as u64,
                shader_location: 4,
            },
        ],
    };

    let depth_stencil = Some(wgpu::DepthStencilState {
        format: parameters.depth_texture.format(),
        depth_write_enabled: true,
        depth_compare: wgpu::CompareFunction::LessEqual,
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    });

    let shader = pipeline_database.shader_from_src(
        device,
        include_shader_src!("crates/appearance-path-tracer-gpu/assets/shaders/gbuffer.wgsl"),
    );
    let pipeline = pipeline_database.render_pipeline(
        device,
        wgpu::RenderPipelineDescriptor {
            label: Some("appearance-path-tracer-gpu::gbuffer"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::TextureFormat::Rgba32Float.into())],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            depth_stencil,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        },
        || {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("appearance-path-tracer-gpu::gbuffer"),
                bind_group_layouts: &[&device.create_bind_group_layout(
                    &wgpu::BindGroupLayoutDescriptor {
                        label: None,
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        }],
                    },
                )],
                push_constant_ranges: &[wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::VERTEX,
                    range: 0..std::mem::size_of::<Mat4>() as u32,
                }],
            })
        },
    );

    let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("appearance-path-tracer-gpu::gbuffer constants"),
        contents: bytemuck::bytes_of(&Constants {
            view_proj: parameters.view_proj,
        }),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group_layout = pipeline.get_bind_group_layout(0);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: constants.as_entire_binding(),
        }],
    });

    let depth_view = parameters
        .depth_texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    {
        let mut rpass = command_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("appearance-path-tracer-gpu::gbuffer"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: parameters.gbuffer_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &bind_group, &[]);

        rpass.set_vertex_buffer(
            0,
            parameters
                .scene_resources
                .vertex_pool()
                .vertex_buffer()
                .slice(..),
        );
        rpass.set_index_buffer(
            parameters
                .scene_resources
                .vertex_pool()
                .index_buffer()
                .slice(..),
            wgpu::IndexFormat::Uint32,
        );

        parameters
            .scene_resources
            .model_instance_iter(|vertex_pool_alloc, transform| {
                rpass.set_push_constants(
                    wgpu::ShaderStages::VERTEX,
                    0,
                    bytemuck::bytes_of(&transform),
                );

                let vertex_slice = &vertex_pool_alloc.slice;
                rpass.draw_indexed(
                    vertex_slice.first_index()..vertex_slice.last_index(),
                    vertex_slice.first_vertex() as i32,
                    0..1,
                );
            });
    }
}
