use std::{collections::HashMap, sync::Arc};

use appearance_asset_database::AssetDatabase;
use appearance_model::Model;
use appearance_world::visible_world_action::VisibleWorldActionType;
use glam::{swizzles::Vec4Swizzles, Mat4, Vec2, Vec3, Vec4};
use tinybvh::{BlasInstance, Bvh, BvhBase, Intersection};
use uuid::Uuid;

use crate::{
    light_sources::point_light::PointLight,
    radiometry::{
        DenselySampledSpectrum, PiecewiseLinearSpectrum, Rgb, RgbColorSpace, RgbIlluminantSpectrum,
        LAMBDA_MAX, LAMBDA_MIN,
    },
};

pub struct GeometryHitData {
    pub position: Vec3,
    pub normal: Vec3,
    pub tex_coord: Option<Vec2>,
}

pub struct GeometryResources {
    model_assets: AssetDatabase<Model>,
    models: HashMap<String, (Arc<Model>, Vec<Uuid>)>,
    model_instances: HashMap<Uuid, Mat4>,

    tlas: Bvh,
    blas_idx_to_mesh_mapping: HashMap<u32, (String, u32, Mat4)>,

    pub point_light: PointLight,
}

impl Default for GeometryResources {
    fn default() -> Self {
        Self::new()
    }
}

impl GeometryResources {
    pub fn new() -> Self {
        let model_assets = AssetDatabase::<Model>::new();

        let light_spectrum = RgbIlluminantSpectrum::new(
            Rgb(Vec3::ONE),
            &RgbColorSpace::srgb(),
            PiecewiseLinearSpectrum::cie_illum_d6500(),
        );
        let light_spectrum = DenselySampledSpectrum::new_from_spectrum(
            &light_spectrum,
            LAMBDA_MIN as u32,
            LAMBDA_MAX as u32,
        );
        let point_light = PointLight::new(Vec3::new(0.0, 5.0, 0.0), light_spectrum, 100.0);

        Self {
            models: HashMap::new(),
            model_instances: HashMap::new(),
            model_assets,
            tlas: Bvh::new(),
            blas_idx_to_mesh_mapping: HashMap::new(),
            point_light,
        }
    }

    pub fn tlas(&self) -> &Bvh {
        &self.tlas
    }

    pub fn handle_visible_world_action(&mut self, action: &VisibleWorldActionType) {
        match action {
            VisibleWorldActionType::SpawnModel(data) => {
                if let Some(model) = self.models.get_mut(data.asset_path()) {
                    model.1.push(data.entity_uuid);
                } else {
                    let model_asset = self.model_assets.get(data.asset_path()).unwrap();
                    self.models.insert(
                        data.asset_path().to_owned(),
                        (model_asset, vec![data.entity_uuid]),
                    );
                }

                self.model_instances
                    .insert(data.entity_uuid, data.transform_matrix);
            }
            VisibleWorldActionType::TransformModel(data) => {
                if let Some(instance_transform) = self.model_instances.get_mut(&data.entity_uuid) {
                    *instance_transform = data.transform_matrix;
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
        model: &Model,
        node: u32,
        parent_transform: Mat4,
        mut blas_idx: u32,
        blas_idx_offset: u32,
        blas_instances: &mut Vec<BlasInstance>,
        blasses: &mut Option<&mut Vec<Arc<dyn BvhBase>>>,
        blas_idx_to_mesh_mapping: &mut HashMap<u32, (String, u32, Mat4)>,
    ) -> u32 {
        let transform = parent_transform * model.nodes[node as usize].transform.get_matrix();

        if let Some(mesh) = &model.nodes[node as usize].mesh {
            if let Some(blasses) = blasses {
                blasses.push(mesh.blas.clone() as Arc<dyn BvhBase>);
            }

            let inv_trans_transform = transform.inverse().transpose();

            blas_idx_to_mesh_mapping.insert(
                blas_instances.len() as u32,
                (model_asset_path.clone(), node, inv_trans_transform),
            );

            blas_instances.push(BlasInstance::new(transform, blas_idx_offset + blas_idx));

            blas_idx += 1;
        }

        for child_node in &model.nodes[node as usize].children {
            blas_idx = Self::rebuild_tlas_rec(
                model_asset_path.clone(),
                model,
                *child_node,
                transform,
                blas_idx,
                blas_idx_offset,
                blas_instances,
                blasses,
                blas_idx_to_mesh_mapping,
            );
        }

        blas_idx
    }

    pub fn rebuild_tlas(&mut self) {
        let mut blasses = vec![];
        let mut blas_instances = vec![];

        let mut blas_idx_to_mesh_mapping = HashMap::new();

        let mut blas_idx_offset = 0;
        for (asset_path, (model, entity_uuids)) in &mut self.models {
            for root_node in &model.root_nodes {
                let mut entity_uuids_indices_to_remove = vec![];

                // Loop over all world instances of the model
                for (i, entity_uuid) in entity_uuids.iter().enumerate() {
                    // If this instance doesn't have a transform anymore, it has been destroyed
                    if let Some(instance_transform) = self.model_instances.get(entity_uuid) {
                        // Assign blasses when on the last instance, also increment the blas idx offset
                        if i == entity_uuids.len() - 1 {
                            blas_idx_offset += Self::rebuild_tlas_rec(
                                asset_path.clone(),
                                model,
                                *root_node,
                                *instance_transform,
                                0,
                                blas_idx_offset,
                                &mut blas_instances,
                                &mut Some(&mut blasses),
                                &mut blas_idx_to_mesh_mapping,
                            );
                        } else {
                            Self::rebuild_tlas_rec(
                                asset_path.clone(),
                                model,
                                *root_node,
                                *instance_transform,
                                0,
                                blas_idx_offset,
                                &mut blas_instances,
                                &mut None,
                                &mut blas_idx_to_mesh_mapping,
                            );
                        };
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

        self.tlas.build_with_blas_instances(blas_instances, blasses);
        self.blas_idx_to_mesh_mapping = blas_idx_to_mesh_mapping;
    }

    pub fn get_hit_data(&self, intersection: &Intersection) -> GeometryHitData {
        let blas_instance = intersection.inst;
        let instance_mapping = self.blas_idx_to_mesh_mapping.get(&blas_instance).unwrap();
        let model = &self.models.get(&instance_mapping.0).unwrap().0;
        let mesh = model.nodes[instance_mapping.1 as usize]
            .mesh
            .as_ref()
            .unwrap();

        let barycentrics = Vec3::new(
            1.0 - intersection.u - intersection.v,
            intersection.u,
            intersection.v,
        );

        let i0 = mesh.indices[(intersection.prim * 3) as usize] as usize;
        let i1 = mesh.indices[(intersection.prim * 3 + 1) as usize] as usize;
        let i2 = mesh.indices[(intersection.prim * 3 + 2) as usize] as usize;

        let p0 = mesh.vertex_positions[i0];
        let p1 = mesh.vertex_positions[i1];
        let p2 = mesh.vertex_positions[i2];
        let position = (p0 * barycentrics.x + p1 * barycentrics.y + p2 * barycentrics.z).xyz();

        let n0 = mesh.vertex_normals[i0];
        let n1 = mesh.vertex_normals[i1];
        let n2 = mesh.vertex_normals[i2];
        let normal = n0 * barycentrics.x + n1 * barycentrics.y + n2 * barycentrics.z;
        let inv_trans_transform = instance_mapping.2;
        let normal = (inv_trans_transform * Vec4::from((normal, 1.0)))
            .xyz()
            .normalize();

        let tex_coord = if !mesh.vertex_tex_coords.is_empty() {
            let t0 = mesh.vertex_tex_coords[i0];
            let t1 = mesh.vertex_tex_coords[i1];
            let t2 = mesh.vertex_tex_coords[i2];
            Some(t0 * barycentrics.x + t1 * barycentrics.y + t2 * barycentrics.z)
        } else {
            None
        };

        GeometryHitData {
            position,
            normal,
            tex_coord,
        }
    }
}
