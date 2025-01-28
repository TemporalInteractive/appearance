use appearance_transform::Transform;
use mesh::Mesh;

pub mod asset;
pub mod mesh;

#[derive(Debug)]
pub struct ModelNode {
    pub name: String,

    pub transform: Transform,
    pub children: Vec<ModelNode>,

    pub mesh: Option<Mesh>,
}

#[derive(Debug)]
pub struct Model {
    pub root_nodes: Vec<ModelNode>,
}
