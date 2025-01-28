use appearance_transform::Transform;

pub struct TransformComponent {
    pub name: String,
    pub transform: Transform,
    pub(crate) marked_for_destroy: bool,
    pub(crate) entity: Option<specs::Entity>,
}

impl TransformComponent {
    pub fn new(name: String, transform: Transform) -> Self {
        Self {
            name,
            transform,
            marked_for_destroy: false,
            entity: None,
        }
    }

    pub fn entity(&self) -> specs::Entity {
        self.entity.unwrap()
    }

    pub fn marked_for_destroy(&self) -> bool {
        self.marked_for_destroy
    }
}

impl specs::Component for TransformComponent {
    type Storage = specs::VecStorage<Self>;
}
