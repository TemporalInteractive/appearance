use std::iter;

use appearance_model::{material::Material, mesh::Mesh, Model, ModelNode};
use appearance_wgpu::wgpu::{self, util::DeviceExt};
use glam::Vec3;

use super::vertex_pool::{VertexPool, VertexPoolAlloc, VertexPoolSlice};

pub struct SceneModel {
    pub root_nodes: Vec<u32>,
    pub materials: Vec<Material>,
    pub meshes: Vec<Mesh>,
    pub vertex_buffers: Vec<wgpu::Buffer>,
    pub index_buffers: Vec<wgpu::Buffer>,
    pub blases: Vec<wgpu::Blas>,
    pub vertex_pool_allocs: Vec<VertexPoolAlloc>,
    pub nodes: Vec<ModelNode>,
}

impl SceneModel {
    pub fn new(
        model: Model,
        vertex_pool: &mut VertexPool,
        command_encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let mut vertex_buffers = vec![];
        let mut index_buffers = vec![];
        let mut blases = vec![];
        let mut vertex_pool_allocs = vec![];

        for mesh in &model.meshes {
            // TODO: make vertex and index buffer global pools
            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh.vertex_positions),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            });

            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::BLAS_INPUT,
            });

            let vertex_pool_alloc = vertex_pool.alloc(
                mesh.vertex_positions.len() as u32,
                mesh.indices.len() as u32,
            );
            vertex_pool.write_vertex_data(
                &mesh.vertex_positions,
                &mesh.vertex_normals,
                &mesh.vertex_tex_coords,
                &mesh.indices,
                vertex_pool_alloc.slice,
                queue,
            );

            let size_desc = wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: mesh.vertex_positions.len() as u32,
                index_format: Some(wgpu::IndexFormat::Uint32),
                index_count: Some(mesh.indices.len() as u32),
                flags: wgpu::AccelerationStructureGeometryFlags::OPAQUE,
            };

            let blas = device.create_blas(
                &wgpu::CreateBlasDescriptor {
                    label: None,
                    flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
                    update_mode: wgpu::AccelerationStructureUpdateMode::Build,
                },
                wgpu::BlasGeometrySizeDescriptors::Triangles {
                    descriptors: vec![size_desc.clone()],
                },
            );

            let triangle_geometry = wgpu::BlasTriangleGeometry {
                size: &size_desc,
                vertex_buffer: &vertex_buffer,
                first_vertex: 0,
                vertex_stride: std::mem::size_of::<Vec3>() as u64,
                index_buffer: Some(&index_buffer),
                first_index: Some(0),
                transform_buffer: None,
                transform_buffer_offset: None,
            };

            let build_entry = wgpu::BlasBuildEntry {
                blas: &blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![triangle_geometry]),
            };

            command_encoder.build_acceleration_structures(iter::once(&build_entry), iter::empty());

            vertex_buffers.push(vertex_buffer);
            index_buffers.push(index_buffer);
            blases.push(blas);
            vertex_pool_allocs.push(vertex_pool_alloc);
        }

        Self {
            root_nodes: model.root_nodes,
            materials: model.materials,
            meshes: model.meshes,
            vertex_buffers,
            index_buffers,
            blases,
            vertex_pool_allocs,
            nodes: model.nodes,
        }
    }
}
