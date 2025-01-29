use appearance_transform::Transform;
use uuid::Uuid;

use crate::visible_world_action::{SpawnModelData, VisibleWorldAction, VisibleWorldActionType};

use super::Component;

#[derive(Debug)]
pub struct ModelComponent {
    pub model: String,
}

impl ModelComponent {
    pub fn new(model: &str) -> Self {
        Self {
            model: model.to_owned(),
        }
    }
}

impl Component for ModelComponent {
    fn visible_world_actions(
        &self,
        transform: &Transform,
        entity_uuid: Uuid,
        visible_world_actions: &mut Vec<VisibleWorldAction>,
    ) {
        visible_world_actions.push(VisibleWorldAction::new(VisibleWorldActionType::SpawnModel(
            SpawnModelData::new(transform.get_matrix(), entity_uuid, &self.model),
        )));
    }
}

impl specs::Component for ModelComponent {
    type Storage = specs::VecStorage<Self>;
}
