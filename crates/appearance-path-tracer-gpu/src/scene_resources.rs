use std::{collections::HashMap, iter};

use appearance_asset_database::AssetDatabase;
use appearance_model::{material::Material, mesh::Mesh, Model, ModelNode};
use appearance_wgpu::wgpu::{self, util::DeviceExt, TlasPackage};
use appearance_world::visible_world_action::VisibleWorldActionType;
use glam::{Mat3, Mat4, Vec3};
use uuid::Uuid;

struct SceneModel {
    root_nodes: Vec<u32>,
    materials: Vec<Material>,
    meshes: Vec<Mesh>,
    vertex_buffers: Vec<wgpu::Buffer>,
    index_buffers: Vec<wgpu::Buffer>,
    blases: Vec<wgpu::Blas>,
    nodes: Vec<ModelNode>,
}

impl SceneModel {
    fn new(
        model: Model,
        command_encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
    ) -> Self {
        let mut vertex_buffers = vec![];
        let mut index_buffers = vec![];
        let mut blases = vec![];

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
        }

        Self {
            root_nodes: model.root_nodes,
            materials: model.materials,
            meshes: model.meshes,
            vertex_buffers,
            index_buffers,
            blases,
            nodes: model.nodes,
        }
    }
}

pub struct SceneResources {
    model_assets: AssetDatabase<Model>,
    models: HashMap<String, (SceneModel, Vec<Uuid>)>,
    model_instances: HashMap<Uuid, Mat4>,

    tlas_package: wgpu::TlasPackage,
}

impl SceneResources {
    pub fn new(device: &wgpu::Device) -> Self {
        let model_assets = AssetDatabase::<Model>::new();

        let tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("appearance-path-tracer-gpu::scene_resources tlas"),
            max_instances: 1024,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        Self {
            tlas_package: TlasPackage::new(tlas),
            model_assets,
            models: HashMap::new(),
            model_instances: HashMap::new(),
        }
    }

    pub fn handle_visible_world_action(
        &mut self,
        action: &VisibleWorldActionType,
        command_encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
    ) {
        match action {
            VisibleWorldActionType::SpawnModel(data) => {
                if let Some(model) = self.models.get_mut(data.asset_path()) {
                    model.1.push(data.entity_uuid);
                } else {
                    let model_asset = self.model_assets.get(data.asset_path()).unwrap();

                    let scene_model =
                        SceneModel::new((*model_asset).clone(), command_encoder, device);

                    self.models.insert(
                        data.asset_path().to_owned(),
                        (scene_model, vec![data.entity_uuid]),
                    );
                }

                self.model_instances
                    .insert(data.entity_uuid, data.transform_matrix);
            }
            _ => log::warn!("Unable to process world action: {:?}.", action),
        }
    }

    pub fn rebuild_tlas(
        &mut self,
        command_encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
    ) {
        // let mut i = 0;
        // for (model, instances) in self.models.values() {
        //     for instance in instances {
        //         let instance_transform = self.model_instances.get(instance).unwrap();
        //         let transform = instance_transform.transpose().to_cols_array()[..12]
        //             .try_into()
        //             .unwrap();

        //         self.tlas_package[i] = Some(wgpu::TlasInstance::new(
        //             &self.scene_components.bottom_level_acceleration_structures[blas_index],
        //             transform,
        //             0,
        //             0xff,
        //         ));
        //         i += 1;
        //     }
        // }

        // command_encoder
        //     .build_acceleration_structures(iter::empty(), iter::once(&self.tlas_package));
    }
}
