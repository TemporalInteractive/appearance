use appearance_transform::Transform;
use uuid::Uuid;

pub struct TransformComponent {
    pub entity_name: String,
    pub transform: Transform,
    uuid: Uuid,
    pub(crate) marked_for_destroy: bool,
    pub(crate) entity: Option<specs::Entity>,
}

impl TransformComponent {
    pub fn new(entity_name: String, transform: Transform) -> Self {
        Self {
            entity_name,
            transform,
            uuid: Uuid::new_v4(),
            marked_for_destroy: false,
            entity: None,
        }
    }

    pub fn entity(&self) -> specs::Entity {
        self.entity.unwrap()
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub fn marked_for_destroy(&self) -> bool {
        self.marked_for_destroy
    }
}

impl specs::Component for TransformComponent {
    type Storage = specs::VecStorage<Self>;
}
