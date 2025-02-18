use appearance_transform::Transform;
use material::Material;
use mesh::Mesh;

pub mod asset;
pub mod material;
pub mod mesh;

pub struct ModelNode {
    pub name: String,

    pub transform: Transform,
    pub children: Vec<u32>,

    pub mesh: Option<Mesh>,
}

pub struct Model {
    pub root_nodes: Vec<u32>,
    pub materials: Vec<Material>,
    pub nodes: Vec<ModelNode>,
}
