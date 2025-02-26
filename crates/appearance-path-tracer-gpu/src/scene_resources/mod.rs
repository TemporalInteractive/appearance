use std::{collections::HashMap, iter};

use appearance_asset_database::AssetDatabase;
use appearance_model::{material::Material, mesh::Mesh, Model, ModelNode};
use appearance_wgpu::wgpu::{self, util::DeviceExt, TlasPackage};
use appearance_world::visible_world_action::VisibleWorldActionType;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use scene_model::SceneModel;
use uuid::Uuid;
use vertex_pool::VertexPool;

mod scene_model;
mod vertex_pool;

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct BlasInstance {
    vertex_pool_slice_index: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

pub struct SceneResources {
    model_assets: AssetDatabase<Model>,
    models: HashMap<String, (SceneModel, Vec<Uuid>)>,
    model_instances: HashMap<Uuid, Mat4>,
    vertex_pool: VertexPool,

    blas_instance_buffer: wgpu::Buffer,

    tlas_package: wgpu::TlasPackage,
    blas_idx_to_mesh_mapping: HashMap<u32, (String, u32, Mat4)>,
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

        let vertex_pool = VertexPool::new(device);

        let blas_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::scene_resources blas_instances"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<BlasInstance>() * 1024) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            model_assets,
            models: HashMap::new(),
            model_instances: HashMap::new(),
            vertex_pool,
            blas_instance_buffer,
            tlas_package: TlasPackage::new(tlas),
            blas_idx_to_mesh_mapping: HashMap::new(),
        }
    }

    pub fn tlas(&self) -> &wgpu::Tlas {
        self.tlas_package.tlas()
    }

    pub fn vertex_pool(&self) -> &VertexPool {
        &self.vertex_pool
    }

    pub fn handle_visible_world_action(
        &mut self,
        action: &VisibleWorldActionType,
        command_encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) {
        match action {
            VisibleWorldActionType::SpawnModel(data) => {
                if let Some(model) = self.models.get_mut(data.asset_path()) {
                    model.1.push(data.entity_uuid);
                } else {
                    let model_asset = self.model_assets.get(data.asset_path()).unwrap();

                    let scene_model = SceneModel::new(
                        (*model_asset).clone(),
                        &mut self.vertex_pool,
                        command_encoder,
                        device,
                        queue,
                    );

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

    fn rebuild_tlas_rec(
        model_asset_path: String,
        model: &SceneModel,
        node: u32,
        parent_transform: Mat4,
        mut blas_idx: u32,
        blas_instances: &mut Vec<wgpu::TlasInstance>,
        blas_idx_to_mesh_mapping: &mut HashMap<u32, (String, u32, Mat4)>,
    ) -> u32 {
        let transform = parent_transform * model.nodes[node as usize].transform.get_matrix();

        if let Some(mesh_idx) = &model.nodes[node as usize].mesh {
            let inv_trans_transform = transform.inverse().transpose();

            blas_idx_to_mesh_mapping.insert(
                blas_instances.len() as u32,
                (model_asset_path.clone(), node, inv_trans_transform),
            );

            let transform = transform.transpose().to_cols_array()[..12]
                .try_into()
                .unwrap();

            let blas = &model.blases[*mesh_idx as usize];
            let vertex_slice_index = model.vertex_pool_allocs[*mesh_idx as usize].index;

            blas_instances.push(wgpu::TlasInstance::new(
                blas,
                transform,
                vertex_slice_index,
                0xff,
            ));

            blas_idx += 1;
        }

        for child_node in &model.nodes[node as usize].children {
            blas_idx = Self::rebuild_tlas_rec(
                model_asset_path.clone(),
                model,
                *child_node,
                transform,
                blas_idx,
                blas_instances,
                blas_idx_to_mesh_mapping,
            );
        }

        blas_idx
    }

    pub fn rebuild_tlas(
        &mut self,
        command_encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
    ) {
        let mut blas_instances = vec![];
        let mut blas_idx_to_mesh_mapping = HashMap::new();

        for (asset_path, (model, entity_uuids)) in &mut self.models {
            for root_node in &model.root_nodes {
                let mut entity_uuids_indices_to_remove = vec![];

                // Loop over all world instances of the model
                for (i, entity_uuid) in entity_uuids.iter().enumerate() {
                    // If this instance doesn't have a transform anymore, it has been destroyed
                    if let Some(instance_transform) = self.model_instances.get(entity_uuid) {
                        Self::rebuild_tlas_rec(
                            asset_path.clone(),
                            model,
                            *root_node,
                            *instance_transform,
                            0,
                            &mut blas_instances,
                            &mut blas_idx_to_mesh_mapping,
                        );
                    } else {
                        entity_uuids_indices_to_remove.push(i);
                    }
                }

                // Remove all entity uuids that have been removed
                entity_uuids_indices_to_remove.sort();
                for (j, i) in entity_uuids_indices_to_remove.iter().enumerate() {
                    entity_uuids.remove(i - j);
                }
            }
        }

        self.blas_idx_to_mesh_mapping = blas_idx_to_mesh_mapping;

        let num_blas_instances = blas_instances.len();
        let tlas_package_instances = self.tlas_package.get_mut_slice(0..1024).unwrap();
        for (i, instance) in blas_instances.into_iter().enumerate() {
            tlas_package_instances[i] = Some(instance);
        }
        for i in num_blas_instances..1024 {
            tlas_package_instances[i] = None;
        }

        self.vertex_pool.write_slices(queue);

        command_encoder
            .build_acceleration_structures(iter::empty(), iter::once(&self.tlas_package));
    }
}
