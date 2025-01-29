use appearance_transform::Transform;
use mesh::Mesh;

pub mod asset;
pub mod mesh;

pub struct ModelNode {
    pub name: String,

    pub transform: Transform,
    pub children: Vec<u32>,

    pub mesh: Option<Mesh>,
}

pub struct Model {
    pub root_nodes: Vec<u32>,
    pub nodes: Vec<ModelNode>,
}
