use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use appearance::appearance_asset_database::AssetDatabase;
use appearance::appearance_camera::Camera;
use appearance::appearance_model::{Model, ModelNode};
use appearance::appearance_render_loop::node::{Node, NodeRenderer};
use appearance::appearance_world::visible_world_action::VisibleWorldActionType;
use appearance::Appearance;
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use tinybvh::{vec_helpers::Vec3Helpers, Ray};
use tinybvh::{BlasInstance, Bvh, BvhBase};
use uuid::Uuid;

struct CameraMatrices {
    inv_view: Mat4,
    inv_proj: Mat4,
}

struct Renderer {
    pixels: Vec<u8>,
    frame_idx: u32,

    models: HashMap<String, (Arc<Model>, Vec<Uuid>)>,
    model_instances: HashMap<Uuid, Mat4>,

    model_assets: AssetDatabase<Model>,
    camera: Camera,

    tlas: Bvh,
}

impl Renderer {
    fn new() -> Self {
        let model_assets = AssetDatabase::<Model>::new();

        Self {
            pixels: Vec::new(),
            frame_idx: 0,
            models: HashMap::new(),
            model_instances: HashMap::new(),
            model_assets,
            camera: Camera::default(),
            tlas: Bvh::new(),
        }
    }

    fn rebuild_tlas_rec(
        node: &ModelNode,
        parent_transform: Mat4,
        mut blas_idx: u32,
        blas_idx_offset: u32,
        blas_instances: &mut Vec<BlasInstance>,
        blasses: &mut Option<&mut Vec<Rc<dyn BvhBase>>>,
    ) -> u32 {
        let transform = parent_transform * node.transform.get_matrix();

        if let Some(mesh) = &node.mesh {
            if let Some(blasses) = blasses {
                blasses.push(mesh.blas.clone() as Rc<dyn BvhBase>);
            }

            blas_instances.push(BlasInstance::new(transform, blas_idx_offset + blas_idx));

            blas_idx += 1;
        }

        for child_node in &node.children {
            blas_idx = Self::rebuild_tlas_rec(
                child_node,
                transform,
                blas_idx,
                blas_idx_offset,
                blas_instances,
                blasses,
            );
        }

        blas_idx
    }

    fn rebuild_tlas(&mut self) {
        let mut blasses = vec![];
        let mut blas_instances = vec![];

        let mut blas_idx_offset = 0;
        for (model, entity_uuids) in self.models.values_mut() {
            for root_node in &model.root_nodes {
                let mut entity_uuids_indices_to_remove = vec![];

                // Loop over all world instances of the model
                for (i, entity_uuid) in entity_uuids.iter().enumerate() {
                    // If this instance doesn't have a transform anymore, it has been destroyed
                    if let Some(instance_transform) = self.model_instances.get(entity_uuid) {
                        // Assign blasses when on the last instance, also increment the blas idx offset
                        if i == entity_uuids.len() - 1 {
                            blas_idx_offset += Self::rebuild_tlas_rec(
                                root_node,
                                *instance_transform,
                                0,
                                blas_idx_offset,
                                &mut blas_instances,
                                &mut Some(&mut blasses),
                            );
                        } else {
                            Self::rebuild_tlas_rec(
                                root_node,
                                *instance_transform,
                                0,
                                blas_idx_offset,
                                &mut blas_instances,
                                &mut None,
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
    }

    fn render_pixel(&mut self, uv: &Vec2, camera_matrices: &CameraMatrices) -> Vec3 {
        let corrected_uv = Vec2::new(uv.x, -uv.y);
        let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
        let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

        let mut ray = Ray::new(origin.xyz(), direction.xyz());

        self.tlas.intersect(&mut ray);
        if ray.hit.t != 1e30 {
            return Vec3::new(1.0, 1.0, 0.0);
        }

        let a = 0.5 * (ray.D.y() + 1.0);
        (1.0 - a) * Vec3::new(1.0, 1.0, 1.0) + a * Vec3::new(0.5, 0.7, 1.0)
    }
}

impl NodeRenderer for Renderer {
    fn visible_world_action(&mut self, action: &VisibleWorldActionType) {
        match action {
            VisibleWorldActionType::CameraUpdate(data) => {
                self.camera.set_near(data.near);
                self.camera.set_far(data.far);
                self.camera.set_fov(data.fov);
                self.camera
                    .transform
                    .set_matrix(data.transform_matrix_bytes);
            }
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
        }
    }

    fn render(&mut self, width: u32, height: u32, assigned_rows: [u32; 2]) -> &[u8] {
        let start_row = assigned_rows[0];
        let end_row = assigned_rows[1];
        let num_rows = end_row - start_row;
        self.pixels.resize((width * num_rows * 4) as usize, 0);

        self.camera.set_aspect_ratio(width as f32 / height as f32);

        let camera_matrices = CameraMatrices {
            inv_view: self.camera.transform.get_matrix(),
            inv_proj: self.camera.get_matrix().inverse(),
        };

        self.rebuild_tlas();

        for local_y in 0..num_rows {
            for local_x in 0..width {
                let x = local_x;
                let y = local_y + start_row;
                let uv = Vec2::new(
                    (x as f32 + 0.5) / width as f32,
                    (y as f32 + 0.5) / height as f32,
                ) * 2.0
                    - 1.0;

                let result = self.render_pixel(&uv, &camera_matrices);

                self.pixels[(local_y * width + local_x) as usize * 4] = (result.x * 255.0) as u8;
                self.pixels[(local_y * width + local_x) as usize * 4 + 1] =
                    (result.y * 255.0) as u8;
                self.pixels[(local_y * width + local_x) as usize * 4 + 2] =
                    (result.z * 255.0) as u8;
                self.pixels[(local_y * width + local_x) as usize * 4 + 3] = 255;
            }
        }

        self.frame_idx += 1;

        &self.pixels
    }
}

pub fn internal_main() -> Result<()> {
    let _appearance = Appearance::new("Render Node");

    let node = Node::new(Renderer::new(), "127.0.0.1:34234")?;
    node.run();

    Ok(())
}
