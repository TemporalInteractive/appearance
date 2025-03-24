use std::{collections::HashMap, iter};

use appearance_asset_database::{asset_paths::resolve_asset_path, AssetDatabase};
use appearance_model::Model;
use appearance_texture::Texture;
use appearance_wgpu::wgpu::{self, TlasPackage};
use appearance_world::visible_world_action::VisibleWorldActionType;
use glam::{Mat4, Vec3};
use material_pool::MaterialPool;
use scene_model::SceneModel;
use sky::Sky;
use uuid::Uuid;
use vertex_pool::{VertexPool, VertexPoolAlloc};

mod material_pool;
pub mod scene_model;
mod sky;
mod vertex_pool;

const MAX_TLAS_INSTANCES: usize = 1024 * 8;

struct TransformWithHistory {
    pub transform: Mat4,
    pub prev_transform: Mat4,
}

impl TransformWithHistory {
    fn new(transform: Mat4) -> Self {
        Self {
            transform,
            prev_transform: transform,
        }
    }

    fn update(&mut self, transform: Mat4) {
        self.prev_transform = self.transform;
        self.transform = transform;
    }
}

pub struct SceneResources {
    model_assets: AssetDatabase<Model>,
    models: HashMap<String, (SceneModel, Vec<Uuid>)>,
    model_instances: HashMap<Uuid, TransformWithHistory>,
    vertex_pool: VertexPool,
    material_pool: MaterialPool,
    sky: Sky,
    frame_idx: u32,

    tlas_package: wgpu::TlasPackage,
    blas_idx_to_mesh_mapping: HashMap<u32, (String, u32, Mat4)>,
}

impl SceneResources {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let model_assets = AssetDatabase::<Model>::new();
        let mut texture_assets = AssetDatabase::<Texture>::new();

        let tlas = device.create_tlas(&wgpu::CreateTlasDescriptor {
            label: Some("appearance-path-tracer-gpu::scene_resources tlas"),
            max_instances: MAX_TLAS_INSTANCES as u32,
            flags: wgpu::AccelerationStructureFlags::PREFER_FAST_TRACE,
            update_mode: wgpu::AccelerationStructureUpdateMode::Build,
        });

        let vertex_pool = VertexPool::new(device);
        let material_pool = MaterialPool::new(device);
        let mut sky = Sky::new(device);

        sky.set_sky_texture(
            &texture_assets
                .get(&resolve_asset_path("::evening_road_01_puresky_4k.hdr", ""))
                .unwrap(),
            device,
            queue,
        );

        Self {
            model_assets,
            models: HashMap::new(),
            model_instances: HashMap::new(),
            vertex_pool,
            material_pool,
            sky,
            frame_idx: 0,
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

    pub fn material_pool(&self) -> &MaterialPool {
        &self.material_pool
    }

    pub fn sky(&self) -> &Sky {
        &self.sky
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
                let resolved_asset_path = resolve_asset_path(data.asset_path(), "");

                if let Some(model) = self.models.get_mut(&resolved_asset_path) {
                    model.1.push(data.entity_uuid);
                } else {
                    let model_asset = self.model_assets.get(&resolved_asset_path).unwrap();

                    let scene_model = SceneModel::new(
                        (*model_asset).clone(),
                        &mut self.vertex_pool,
                        &mut self.material_pool,
                        command_encoder,
                        device,
                        queue,
                    );

                    self.models
                        .insert(resolved_asset_path, (scene_model, vec![data.entity_uuid]));
                }

                self.model_instances.insert(
                    data.entity_uuid,
                    TransformWithHistory::new(data.transform_matrix),
                );
            }
            VisibleWorldActionType::TransformModel(data) => {
                if let Some(instance_transform) = self.model_instances.get_mut(&data.entity_uuid) {
                    instance_transform.update(data.transform_matrix);
                } else {
                    log::warn!("Failed to update model instance transform.");
                }
            }
            VisibleWorldActionType::DestroyModel(data) => {
                self.model_instances.remove(&data.entity_uuid);
            }
            VisibleWorldActionType::Clear(_) => {
                self.models.clear();
            }
            _ => log::warn!("Unable to process world action: {:?}.", action),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rebuild_tlas_rec(
        model_asset_path: String,
        model: &SceneModel,
        node: u32,
        parent_transform: Mat4,
        mut blas_idx: u32,
        blas_instances: &mut Vec<wgpu::TlasInstance>,
        blas_idx_to_mesh_mapping: &mut HashMap<u32, (String, u32, Mat4)>,
        vertex_pool: &mut VertexPool,
    ) -> u32 {
        let transform = parent_transform * model.nodes[node as usize].transform.get_matrix();

        if let Some(mesh_idx) = &model.nodes[node as usize].mesh {
            let inv_trans_transform = transform.inverse().transpose();

            blas_idx_to_mesh_mapping.insert(
                blas_instances.len() as u32,
                (model_asset_path.clone(), node, inv_trans_transform),
            );

            let transform4x3 = transform.transpose().to_cols_array()[..12]
                .try_into()
                .unwrap();

            let blas = &model.blases[*mesh_idx as usize];
            let vertex_slice_index = model.vertex_pool_allocs[*mesh_idx as usize].index;

            blas_instances.push(wgpu::TlasInstance::new(
                blas,
                transform4x3,
                vertex_slice_index,
                0xff,
            ));

            vertex_pool.submit_slice_instance(
                vertex_slice_index,
                transform,
                model.is_emissive[*mesh_idx as usize],
            );

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
                vertex_pool,
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
                            instance_transform.transform,
                            0,
                            &mut blas_instances,
                            &mut blas_idx_to_mesh_mapping,
                            &mut self.vertex_pool,
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
        let tlas_package_instances = self
            .tlas_package
            .get_mut_slice(0..MAX_TLAS_INSTANCES)
            .unwrap();
        for (i, instance) in blas_instances.into_iter().enumerate() {
            tlas_package_instances[i] = Some(instance);
        }
        for i in num_blas_instances..MAX_TLAS_INSTANCES {
            tlas_package_instances[i] = None;
        }

        self.vertex_pool.write_slices(queue);
        self.material_pool.write_materials(queue);

        command_encoder
            .build_acceleration_structures(iter::empty(), iter::once(&self.tlas_package));
    }

    pub fn end_frame(&mut self) {
        self.frame_idx += 1;
        self.vertex_pool.end_frame();

        // TODO: this kind of logic shouldn't be here, just for testing
        // self.sky.sun_info.direction = Vec3::new(
        //     (self.frame_idx as f32 / 100.0).cos() * -0.2,
        //     -1.0,
        //     (self.frame_idx as f32 / 100.0).sin() * 0.3,
        // )
        // .normalize();
    }

    fn model_instance_iter_rec<F: FnMut(&VertexPoolAlloc, Mat4, Mat4)>(
        f: &mut F,
        model_asset_path: String,
        model: &SceneModel,
        node: u32,
        parent_transform: Mat4,
        prev_parent_transform: Mat4,
    ) {
        let transform = parent_transform * model.nodes[node as usize].transform.get_matrix();
        let prev_transform =
            prev_parent_transform * model.nodes[node as usize].transform.get_matrix();

        if let Some(mesh_idx) = &model.nodes[node as usize].mesh {
            let vertex_slice = &model.vertex_pool_allocs[*mesh_idx as usize];

            f(vertex_slice, transform, prev_transform);
        }

        for child_node in &model.nodes[node as usize].children {
            Self::model_instance_iter_rec(
                f,
                model_asset_path.clone(),
                model,
                *child_node,
                transform,
                prev_transform,
            );
        }
    }

    pub fn model_instance_iter<F: FnMut(&VertexPoolAlloc, Mat4, Mat4)>(&self, mut f: F) {
        for (asset_path, (model, entity_uuids)) in &self.models {
            for root_node in &model.root_nodes {
                // Loop over all world instances of the model
                for entity_uuid in entity_uuids {
                    // If this instance doesn't have a transform anymore, it has been destroyed
                    if let Some(instance_transform) = self.model_instances.get(entity_uuid) {
                        Self::model_instance_iter_rec(
                            &mut f,
                            asset_path.clone(),
                            model,
                            *root_node,
                            instance_transform.transform,
                            instance_transform.prev_transform,
                        );
                    }
                }
            }
        }
    }
}
