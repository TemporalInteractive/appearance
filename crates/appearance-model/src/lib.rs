use appearance_transform::Transform;
use mesh::Mesh;

pub mod asset;
pub mod mesh;

pub struct ModelNode {
    pub name: String,

    pub transform: Transform,
    pub children: Vec<ModelNode>,

    pub mesh: Option<Mesh>,
}

pub struct Model {
    pub root_nodes: Vec<ModelNode>,
}
