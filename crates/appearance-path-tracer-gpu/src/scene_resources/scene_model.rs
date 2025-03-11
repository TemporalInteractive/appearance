use std::iter;

use appearance_model::{mesh::PackedVertex, Model, ModelNode};
use appearance_wgpu::wgpu;

use super::{
    material_pool::MaterialPool,
    vertex_pool::{VertexPool, VertexPoolAlloc, VertexPoolWriteData},
};

pub struct SceneModel {
    pub root_nodes: Vec<u32>,
    pub blases: Vec<wgpu::Blas>,
    pub is_emissive: Vec<bool>,
    pub vertex_pool_allocs: Vec<VertexPoolAlloc>,
    pub nodes: Vec<ModelNode>,
}

impl SceneModel {
    pub fn new(
        model: Model,
        vertex_pool: &mut VertexPool,
        material_pool: &mut MaterialPool,
        command_encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Self {
        let mut blases = vec![];
        let mut is_emissive = vec![];
        let mut vertex_pool_allocs = vec![];

        let material_idx = material_pool.material_count();
        for material in &model.materials {
            material_pool.alloc_material(material, device, queue);
        }

        for mesh in &model.meshes {
            let vertex_pool_alloc = vertex_pool.alloc(
                mesh.packed_vertices.len() as u32,
                mesh.indices.len() as u32,
                material_idx as u32,
            );
            vertex_pool.write_vertex_data(
                &VertexPoolWriteData {
                    packed_vertices: &mesh.packed_vertices,
                    indices: &mesh.indices,
                    triangle_material_indices: &mesh.triangle_material_indices,
                },
                vertex_pool_alloc.slice,
                queue,
            );

            let size_desc = wgpu::BlasTriangleGeometrySizeDescriptor {
                vertex_format: wgpu::VertexFormat::Float32x3,
                vertex_count: mesh.packed_vertices.len() as u32,
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
                vertex_buffer: vertex_pool.vertex_buffer(),
                first_vertex: vertex_pool_alloc.slice.first_vertex(),
                vertex_stride: std::mem::size_of::<PackedVertex>() as u64,
                index_buffer: Some(vertex_pool.index_buffer()),
                first_index: Some(vertex_pool_alloc.slice.first_index()),
                transform_buffer: None,
                transform_buffer_offset: None,
            };

            let build_entry = wgpu::BlasBuildEntry {
                blas: &blas,
                geometry: wgpu::BlasGeometries::TriangleGeometries(vec![triangle_geometry]),
            };

            command_encoder.build_acceleration_structures(iter::once(&build_entry), iter::empty());

            blases.push(blas);
            is_emissive.push(mesh.is_emissive);
            vertex_pool_allocs.push(vertex_pool_alloc);
        }

        Self {
            root_nodes: model.root_nodes,
            blases,
            is_emissive,
            vertex_pool_allocs,
            nodes: model.nodes,
        }
    }
}
