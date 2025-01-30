use rayon::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use appearance::appearance_asset_database::AssetDatabase;
use appearance::appearance_camera::Camera;
use appearance::appearance_model::Model;
use appearance::appearance_render_loop::node::{Node, NodeRenderer};
use appearance::appearance_world::visible_world_action::VisibleWorldActionType;
use appearance::Appearance;
use clap::{arg, command, Parser};
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use tinybvh::{vec_helpers::Vec3Helpers, Ray};
use tinybvh::{BlasInstance, Bvh, BvhBase, Intersection};
use uuid::Uuid;

struct CameraMatrices {
    inv_view: Mat4,
    inv_proj: Mat4,
}

struct Renderer {
    pixels: Arc<Mutex<Vec<u8>>>,
    frame_idx: u32,

    models: HashMap<String, (Arc<Model>, Vec<Uuid>)>,
    model_instances: HashMap<Uuid, Mat4>,

    model_assets: AssetDatabase<Model>,
    camera: Camera,

    tlas: Bvh,
    blas_idx_to_mesh_mapping: HashMap<u32, (String, u32, Mat4)>,
}

impl Renderer {
    fn new() -> Self {
        let model_assets = AssetDatabase::<Model>::new();

        Self {
            pixels: Arc::new(Mutex::new(Vec::new())),
            frame_idx: 0,
            models: HashMap::new(),
            model_instances: HashMap::new(),
            model_assets,
            camera: Camera::default(),
            tlas: Bvh::new(),
            blas_idx_to_mesh_mapping: HashMap::new(),
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

    fn rebuild_tlas(&mut self) {
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

    fn get_hit_data(&self, intersection: &Intersection) -> (Vec3, Vec3, Option<Vec2>) {
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
        let position = p0 * barycentrics.x + p1 * barycentrics.y + p2 * barycentrics.z;

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

        (position.xyz(), normal, tex_coord)
    }

    fn render_pixel(&self, uv: &Vec2, camera_matrices: &CameraMatrices) -> Vec3 {
        let corrected_uv = Vec2::new(uv.x, -uv.y);
        let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
        let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

        let mut ray = Ray::new(origin.xyz(), direction.xyz());

        for _ in 0..1 {
            self.tlas.intersect(&mut ray);
        }
        if ray.hit.t != 1e30 {
            let (_position, normal, _tex_coord) = self.get_hit_data(&ray.hit);

            return normal * 0.5 + 0.5;
        }

        let a = 0.5 * (ray.D.y() + 1.0);
        (1.0 - a) * Vec3::new(1.0, 1.0, 1.0) + a * Vec3::new(0.5, 0.7, 1.0)

        //Vec3::new(uv.x, uv.y, 0.0) * 0.5 + 0.5
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

    fn render<F: Fn(&[u8])>(
        &mut self,
        width: u32,
        height: u32,
        start_row: u32,
        end_row: u32,
        result_callback: F,
    ) {
        let num_rows = end_row - start_row;
        if let Ok(mut pixels) = self.pixels.lock() {
            pixels.resize((width * num_rows * 4) as usize, 128);

            // result_callback(pixels.as_ref());
            // return;
        }

        self.camera.set_aspect_ratio(width as f32 / height as f32);

        let camera_matrices = CameraMatrices {
            inv_view: self.camera.transform.get_matrix(),
            inv_proj: self.camera.get_matrix().inverse(),
        };

        self.rebuild_tlas();

        let _ = (0..(num_rows * width))
            .collect::<Vec<u32>>()
            .par_iter()
            .map(|i| {
                let local_y = i / width;
                let local_x = i % width;
                let x = local_x;
                let y = local_y + start_row;
                let uv = Vec2::new(
                    (x as f32 + 0.5) / width as f32,
                    (y as f32 + 0.5) / height as f32,
                ) * 2.0
                    - 1.0;

                let result = self.render_pixel(&uv, &camera_matrices);

                if let Ok(mut pixels) = self.pixels.lock() {
                    pixels[(local_y * width + local_x) as usize * 4] = (result.x * 255.0) as u8;
                    pixels[(local_y * width + local_x) as usize * 4 + 1] = (result.y * 255.0) as u8;
                    pixels[(local_y * width + local_x) as usize * 4 + 2] = (result.z * 255.0) as u8;
                    pixels[(local_y * width + local_x) as usize * 4 + 3] = 255;
                }
            })
            .collect::<Vec<_>>();

        // for local_y in 0..num_rows {
        //     for local_x in 0..width {
        //         let x = local_x;
        //         let y = local_y + start_row;
        //         let uv = Vec2::new(
        //             (x as f32 + 0.5) / width as f32,
        //             (y as f32 + 0.5) / height as f32,
        //         ) * 2.0
        //             - 1.0;

        //         let result = self.render_pixel(&uv, &camera_matrices);

        //         if let Ok(mut pixels) = self.pixels.lock() {
        //             pixels[(local_y * width + local_x) as usize * 4] = (result.x * 255.0) as u8;
        //             pixels[(local_y * width + local_x) as usize * 4 + 1] = (result.y * 255.0) as u8;
        //             pixels[(local_y * width + local_x) as usize * 4 + 2] = (result.z * 255.0) as u8;
        //             pixels[(local_y * width + local_x) as usize * 4 + 3] = 255;
        //         }
        //     }
        // }

        self.frame_idx += 1;

        if let Ok(pixels) = self.pixels.lock() {
            result_callback(pixels.as_ref());
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Ip 169.254.187.239
    #[arg(long, default_value_t = String::from("169.254.187.239"))]
    host_ip: String,

    /// Host port
    #[arg(long, default_value_t = String::from("34234"))]
    host_port: String,

    /// Node port
    #[arg(long, default_value_t = String::from("34235"))]
    node_port: String,
}

pub fn internal_main() -> Result<()> {
    let _appearance = Appearance::new("Render Node");

    let args = Args::parse();
    let node = Node::new(
        Renderer::new(),
        &args.host_ip,
        &args.host_port,
        &args.node_port,
    )?;
    node.run();

    Ok(())
}
