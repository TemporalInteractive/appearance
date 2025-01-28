use appearance_camera::Camera;
use appearance_transform::Transform;
use components::TransformComponent;
use glam::Vec3;
use specs::{Builder, WorldExt};
use visible_world_action::{CameraUpdateData, VisibleWorldAction, VisibleWorldActionType};

pub mod components;
pub mod visible_world_action;

pub struct EntityBuilder<'a> {
    builder: specs::EntityBuilder<'a>,
}

impl<'a> EntityBuilder<'a> {
    fn new(ecs: &'a mut specs::World, name: &str, transform: Transform) -> Self {
        let builder = ecs
            .create_entity()
            .with(TransformComponent::new(name.to_owned(), transform));

        Self { builder }
    }

    pub fn with<T: specs::Component + Send + Sync>(self, c: T) -> Self {
        Self {
            builder: self.builder.with(c),
        }
    }
}

/// The world is how the game is percieved by the host, including not only visual but also gameplay elements
pub struct World {
    ecs: specs::World,
    entities_marked_for_destroy: Vec<specs::Entity>,
    camera: Camera,

    visible_world_actions: Vec<VisibleWorldAction>,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            ecs: specs::World::new(),
            entities_marked_for_destroy: Vec::new(),
            camera: Camera::new(
                Transform::from_translation(Vec3::new(0.0, 0.0, -5.0)),
                60.0,
                0.1,
                100.0,
                1.0,
            ),
            visible_world_actions: Vec::new(),
        }
    }

    pub fn create_entity<F>(
        &mut self,
        name: &str,
        transform: Transform,
        builder_pattern: F,
    ) -> specs::Entity
    where
        F: Fn(EntityBuilder<'_>) -> EntityBuilder<'_>,
    {
        appearance_profiling::profile_function!();

        let entity = {
            let builder = builder_pattern(EntityBuilder::new(&mut self.ecs, name, transform));
            builder.builder.build()
        };

        self.entities_mut::<TransformComponent>()
            .get_mut(entity)
            .unwrap()
            .entity = Some(entity);

        entity
    }

    pub fn destroy_entity(&mut self, entity: specs::Entity) {
        appearance_profiling::profile_function!();

        self.entities_mut::<TransformComponent>()
            .get_mut(entity)
            .unwrap()
            .marked_for_destroy = true;
        self.entities_marked_for_destroy.push(entity);
    }

    pub fn entities<T: specs::Component>(&self) -> specs::ReadStorage<T> {
        self.ecs.read_storage()
    }

    pub fn entities_mut<T: specs::Component>(&self) -> specs::WriteStorage<T> {
        self.ecs.write_storage()
    }

    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    pub fn camera_mut<F: FnMut(&mut Camera)>(&mut self, mut callback: F) {
        callback(&mut self.camera);

        self.visible_world_actions.push(VisibleWorldAction::new(
            VisibleWorldActionType::CameraUpdate(CameraUpdateData {
                fov: self.camera.get_fov(),
                near: self.camera.get_near(),
                far: self.camera.get_far(),
                transform_matrix_bytes: self.camera.transform.get_matrix(),
                _padding: 0,
            }),
        ));
    }

    pub fn get_visible_world_actions(&self) -> &[VisibleWorldAction] {
        &self.visible_world_actions
    }

    pub fn update(&mut self) {
        appearance_profiling::profile_function!();

        self.visible_world_actions.clear();

        for entity in &self.entities_marked_for_destroy {
            self.ecs.delete_entity(*entity).unwrap();
        }
        self.entities_marked_for_destroy.clear();
    }
}
