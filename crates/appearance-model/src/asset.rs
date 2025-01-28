use appearance_asset_database::Asset;
use appearance_transform::Transform;
use glam::{Quat, Vec2, Vec3, Vec4};

use crate::{mesh::Mesh, Model, ModelNode};

impl Asset for Model {
    fn load(_file_path: &str, data: Vec<u8>) -> Self {
        appearance_profiling::profile_function!();

        let (document, buffers, images) = gltf::import_slice(&data).expect("Failed to load model.");

        let mut root_nodes = Vec::new();
        if let Some(scene) = document.default_scene() {
            for root_node in scene.nodes() {
                root_nodes.push(process_nodes_recursive(
                    &document, &root_node, &buffers, &images,
                ));
            }
        }

        Model { root_nodes }
    }
}

fn process_nodes_recursive(
    document: &gltf::Document,
    node: &gltf::Node,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
) -> ModelNode {
    let mut root_node = process_node(document, node, buffers, images);

    for child in node.children() {
        root_node
            .children
            .push(process_nodes_recursive(document, &child, buffers, images));
    }
    root_node
}

fn process_node(
    _document: &gltf::Document,
    node: &gltf::Node,
    buffers: &[gltf::buffer::Data],
    _images: &[gltf::image::Data],
) -> ModelNode {
    let (translation, rotation, scale) = node.transform().decomposed();
    let translation = Vec3::new(translation[0], translation[1], translation[2]);
    let rotation = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]);
    let scale = Vec3::new(scale[0], scale[1], scale[2]);
    let transform = Transform::new(translation, rotation, scale);

    let mut node_mesh = None;

    if let Some(mesh) = node.mesh() {
        let mut mesh_vertex_positions = vec![];
        let mut mesh_vertex_tex_coords = vec![];
        let mut mesh_vertex_normals = vec![];
        let mut mesh_vertex_tangents = vec![];
        let mut mesh_indices = vec![];
        let mut material_idx = 0;

        for primitive in mesh.primitives() {
            if primitive.mode() == gltf::mesh::Mode::Triangles {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                let mut vertex_positions = {
                    let iter = reader
                        .read_positions()
                        .expect("Failed to process mesh node. (Vertices must have positions)");

                    iter.map(|arr| -> Vec4 { Vec4::from((Vec3::from(arr), 0.0)) })
                        .collect::<Vec<_>>()
                };

                let indices = reader
                    .read_indices()
                    .map(|read_indices| read_indices.into_u32().collect::<Vec<_>>())
                    .expect("Failed to process mesh node. (Indices are required)");

                let mut vertex_tex_coords = if let Some(tex_coords) = reader.read_tex_coords(0) {
                    tex_coords
                        .into_f32()
                        .map(|normal| -> Vec2 { Vec2::from(normal) })
                        .collect()
                } else {
                    vec![]
                };

                let mut vertex_normals = if let Some(normals) = reader.read_normals() {
                    normals
                        .into_iter()
                        .map(|normal| -> Vec3 { Vec3::from(normal) })
                        .collect()
                } else {
                    vec![]
                };

                let mut vertex_tangents = if let Some(tangents) = reader.read_tangents() {
                    tangents
                        .into_iter()
                        .map(|tangent| -> Vec4 { Vec4::from(tangent) })
                        .collect()
                } else {
                    vec![]
                };

                let mut indices = indices
                    .into_iter()
                    .map(|index| index + mesh_vertex_positions.len() as u32)
                    .collect::<Vec<u32>>();
                mesh_vertex_positions.append(&mut vertex_positions);
                mesh_vertex_tex_coords.append(&mut vertex_tex_coords);
                mesh_vertex_normals.append(&mut vertex_normals);
                mesh_vertex_tangents.append(&mut vertex_tangents);
                mesh_indices.append(&mut indices);

                material_idx = primitive.material().index().unwrap_or(0);
            } else {
                panic!("Only triangles are supported.");
            }
        }

        let mut mesh = Mesh::new(
            mesh_vertex_positions,
            mesh_vertex_normals,
            mesh_vertex_tangents,
            mesh_vertex_tex_coords,
            mesh_indices,
            material_idx as u32,
        );
        if !mesh.has_normals() {
            mesh.generate_normals();
        }
        if !mesh.has_tangents() {
            mesh.generate_tangents();
        }

        node_mesh = Some(mesh);
    }

    ModelNode {
        name: node.name().unwrap_or("Unnamed").to_owned(),
        transform,
        mesh: node_mesh,
        children: vec![],
    }
}
