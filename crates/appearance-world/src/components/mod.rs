pub mod model;
use appearance_transform::Transform;
pub use model::*;
pub mod transform;
pub use transform::*;
use uuid::Uuid;

use crate::visible_world_action::VisibleWorldAction;

pub trait Component: specs::Component {
    fn visible_world_actions(
        &self,
        transform: &Transform,
        entity_uuid: Uuid,
        visible_world_actions: &mut Vec<VisibleWorldAction>,
    );
}
