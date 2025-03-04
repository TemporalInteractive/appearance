use std::sync::Arc;

use anyhow::Result;
use appearance_asset_database::Asset;
use appearance_texture::{Texture, TextureCreateDesc, TextureFormat};
use appearance_transform::Transform;
use glam::{Quat, Vec2, Vec3, Vec4, Vec4Swizzles};
use gltf::material::AlphaMode;
use image::DynamicImage;
use uuid::Uuid;

use crate::{material::Material, mesh::Mesh, Model, ModelNode};

impl Asset for Model {
    fn load(_file_path: &str, data: &[u8]) -> Result<Self> {
        appearance_profiling::profile_function!();

        let (document, buffers, images) = gltf::import_slice(data)?;

        let mut internal_images = vec![None; document.textures().len()];
        let mut materials = vec![Material::default(); document.materials().len()];
        if materials.is_empty() {
            materials.push(Material::default());
        }

        let mut root_nodes = Vec::new();
        let mut nodes = Vec::new();
        let mut meshes = Vec::new();
        meshes.resize_with(document.meshes().len(), Default::default);

        if let Some(scene) = document.default_scene() {
            for root_node in scene.nodes() {
                root_nodes.push(nodes.len() as u32);
                process_nodes_recursive(
                    &document,
                    &root_node,
                    &buffers,
                    &images,
                    &mut nodes,
                    &mut internal_images,
                    &mut materials,
                    &mut meshes,
                );
            }
        }

        let meshes = meshes.into_iter().map(|mesh| mesh.unwrap()).collect();

        Ok(Model {
            root_nodes,
            materials,
            meshes,
            nodes,
            uuid: Uuid::new_v4(),
        })
    }

    fn uuid(&self) -> Uuid {
        self.uuid
    }
}

#[allow(clippy::too_many_arguments)]
fn process_nodes_recursive(
    document: &gltf::Document,
    node: &gltf::Node,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    nodes: &mut Vec<ModelNode>,
    internal_images: &mut Vec<Option<Arc<Texture>>>,
    materials: &mut Vec<Material>,
    meshes: &mut Vec<Option<Mesh>>,
) {
    nodes.push(process_node(
        document,
        node,
        buffers,
        images,
        internal_images,
        materials,
        meshes,
    ));
    let node_idx = nodes.len() - 1;

    for child in node.children() {
        let child_idx = nodes.len() as u32;
        nodes[node_idx].children.push(child_idx);
        process_nodes_recursive(
            document,
            &child,
            buffers,
            images,
            nodes,
            internal_images,
            materials,
            meshes,
        );
    }
}

fn process_node(
    document: &gltf::Document,
    node: &gltf::Node,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    internal_images: &mut [Option<Arc<Texture>>],
    materials: &mut [Material],
    meshes: &mut [Option<Mesh>],
) -> ModelNode {
    appearance_profiling::profile_function!();

    let (translation, rotation, scale) = node.transform().decomposed();
    let translation = Vec3::new(translation[0], translation[1], translation[2]);
    let rotation = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]);
    let scale = Vec3::new(scale[0], scale[1], scale[2]);
    let transform = Transform::new(translation, rotation, scale);

    let mut node_mesh = None;

    if let Some(mesh) = node.mesh() {
        let mesh_idx = mesh.index();
        if meshes[mesh_idx].is_none() {
            let mut mesh_vertex_positions = vec![];
            let mut mesh_vertex_tex_coords = vec![];
            let mut mesh_vertex_normals = vec![];
            let mut mesh_vertex_tangents = vec![];
            let mut mesh_triangle_material_indices = vec![];
            let mut mesh_indices = vec![];
            let mut opaque = true;

            for primitive in mesh.primitives() {
                if primitive.mode() == gltf::mesh::Mode::Triangles {
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                    let mut vertex_positions = {
                        let iter = reader
                            .read_positions()
                            .expect("Failed to process mesh node. (Vertices must have positions)");

                        iter.map(|arr| -> Vec3 { Vec3::from(arr) })
                            .collect::<Vec<_>>()
                    };

                    let indices = reader
                        .read_indices()
                        .map(|read_indices| read_indices.into_u32().collect::<Vec<_>>())
                        .expect("Failed to process mesh node. (Indices are required)");

                    let mut vertex_tex_coords = if let Some(tex_coords) = reader.read_tex_coords(0)
                    {
                        tex_coords
                            .into_f32()
                            .map(|tex_coord| -> Vec2 { Vec2::from(tex_coord) })
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

                    let num_triangles = indices.len() / 3;

                    let mut indices = indices
                        .into_iter()
                        .map(|index| index + mesh_vertex_positions.len() as u32)
                        .collect::<Vec<u32>>();
                    mesh_vertex_positions.append(&mut vertex_positions);
                    mesh_vertex_tex_coords.append(&mut vertex_tex_coords);
                    mesh_vertex_normals.append(&mut vertex_normals);
                    mesh_vertex_tangents.append(&mut vertex_tangents);
                    mesh_indices.append(&mut indices);

                    let prim_material = primitive.material();
                    let pbr = prim_material.pbr_metallic_roughness();
                    let material_idx = primitive.material().index().unwrap_or(0);

                    mesh_triangle_material_indices
                        .append(&mut vec![material_idx as u32; num_triangles]);

                    let material = &mut materials[material_idx];
                    if material.index.is_none() {
                        material.index = Some(material_idx);

                        material.color = Vec4::from(pbr.base_color_factor()).xyz();
                        material.metallic = pbr.metallic_factor();
                        material.roughness = pbr.roughness_factor();
                        material.emission = Vec3::from(prim_material.emissive_factor());
                        material.emission *= prim_material.emissive_strength().unwrap_or(1.0);

                        if let Some(volume) = prim_material.volume() {
                            // TODO: not 100 percent sure this is correct
                            material.absorption = (Vec3::ONE
                                - Vec3::from(volume.attenuation_color()))
                                / volume.attenuation_distance();
                        }
                        if let Some(transmission) = prim_material.transmission() {
                            material.transmission = transmission.transmission_factor();
                            if let Some(tex) = transmission.transmission_texture() {
                                material.transmission_texture = Some(process_tex(
                                    document,
                                    images,
                                    internal_images,
                                    &tex.texture(),
                                    tex.texture().name().unwrap_or("Transmission"),
                                ));
                            }
                        }
                        material.eta = 1.0 / prim_material.ior().unwrap_or(1.5);

                        material.subsurface = 0.0; // TODO
                        if let Some(specular) = prim_material.specular() {
                            material.specular = specular.specular_factor();
                            material.specular_tint = Vec3::from(specular.specular_color_factor());
                        }
                        if let Some(clearcoat) = prim_material.clearcoat() {
                            material.clearcoat = clearcoat.clearcoat_factor();
                            if let Some(tex) = clearcoat.clearcoat_texture() {
                                material.clearcoat_texture = Some(process_tex(
                                    document,
                                    images,
                                    internal_images,
                                    &tex.texture(),
                                    tex.texture().name().unwrap_or("Clearcoat"),
                                ));
                            }
                            material.clearcoat_roughness = clearcoat.clearcoat_roughness_factor();
                            if let Some(tex) = clearcoat.clearcoat_roughness_texture() {
                                material.clearcoat_roughness_texture = Some(process_tex(
                                    document,
                                    images,
                                    internal_images,
                                    &tex.texture(),
                                    tex.texture().name().unwrap_or("Clearcoat Roughness"),
                                ));
                            }
                        }
                        if let Some(sheen) = prim_material.sheen() {
                            material.sheen = sheen.sheen_roughness_factor();
                            material.sheen_tint = Vec3::from(sheen.sheen_color_factor());
                        }

                        material.alpha_cutoff = prim_material.alpha_cutoff().unwrap_or(0.0);
                        material.is_opaque = prim_material.alpha_mode() == AlphaMode::Opaque
                            || material.alpha_cutoff == 0.0;

                        if let Some(tex) = pbr.base_color_texture() {
                            material.color_texture = Some(process_tex(
                                document,
                                images,
                                internal_images,
                                &tex.texture(),
                                tex.texture().name().unwrap_or("Color"),
                            ));
                        }

                        if let Some(tex) = prim_material.normal_texture() {
                            material.normal_texture = Some(process_tex(
                                document,
                                images,
                                internal_images,
                                &tex.texture(),
                                tex.texture().name().unwrap_or("Normal"),
                            ));
                            material.normal_scale = tex.scale();
                        }

                        if let Some(tex) = pbr.metallic_roughness_texture() {
                            material.metallic_roughness_texture = Some(process_tex(
                                document,
                                images,
                                internal_images,
                                &tex.texture(),
                                tex.texture().name().unwrap_or("Metallic Roughness"),
                            ));
                        }

                        if let Some(tex) = prim_material.emissive_texture() {
                            material.emission_texture = Some(process_tex(
                                document,
                                images,
                                internal_images,
                                &tex.texture(),
                                tex.texture().name().unwrap_or("Emission"),
                            ));
                        }
                    }

                    opaque = opaque && material.is_opaque;
                } else {
                    panic!("Only triangles are supported.");
                }
            }

            let mut mesh = Mesh::new(
                mesh_vertex_positions,
                mesh_vertex_normals,
                mesh_vertex_tangents,
                mesh_vertex_tex_coords,
                mesh_triangle_material_indices,
                mesh_indices,
                opaque,
            );
            if !mesh.has_normals() {
                mesh.generate_normals();
            }
            if !mesh.has_tangents() {
                mesh.generate_tangents();
            }

            meshes[mesh_idx] = Some(mesh);
        }

        node_mesh = Some(mesh_idx as u32);
    }

    ModelNode {
        name: node.name().unwrap_or("Unnamed").to_owned(),
        transform,
        mesh: node_mesh,
        children: vec![],
    }
}

fn process_tex(
    document: &gltf::Document,
    images: &[gltf::image::Data],
    internal_images: &mut [Option<Arc<Texture>>],
    texture: &gltf::Texture,
    name: &str,
) -> Arc<Texture> {
    appearance_profiling::profile_function!();

    match texture.source().source() {
        gltf::image::Source::View { .. } => {
            let texture_idx = texture.index();
            let image_idx = document
                .textures()
                .nth(texture_idx)
                .unwrap()
                .source()
                .index();

            if internal_images[image_idx].is_none() {
                let data = images[image_idx].clone();

                let create_desc = if data.format == gltf::image::Format::R8G8B8 {
                    let dynamic_image = DynamicImage::ImageRgb8(
                        image::RgbImage::from_raw(data.width, data.height, data.pixels).unwrap(),
                    );
                    let image = dynamic_image.to_rgba8();

                    TextureCreateDesc {
                        name: Some(name.to_owned()),
                        width: data.width,
                        height: data.height,
                        format: TextureFormat::Rgba8Unorm,
                        data: image.as_raw().clone().into_boxed_slice(),
                    }
                } else {
                    let format = match data.format {
                        gltf::image::Format::R8G8B8A8 => TextureFormat::Rgba8Unorm,
                        gltf::image::Format::R8G8 => TextureFormat::Rg8Unorm,
                        gltf::image::Format::R8 => TextureFormat::R8Unorm,
                        _ => panic!("Unsupported image type: {:?}.", data.format),
                    };

                    TextureCreateDesc {
                        name: Some(name.to_owned()),
                        width: data.width,
                        height: data.height,
                        format,
                        data: data.pixels.into_boxed_slice(),
                    }
                };

                internal_images[image_idx] = Some(Arc::new(Texture::new(create_desc)));
            }

            internal_images[image_idx].as_ref().unwrap().clone()
        }
        gltf::image::Source::Uri { .. } => todo!(),
    }
}
