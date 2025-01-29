use appearance_camera::Camera;
use appearance_transform::Transform;
use components::{Component, ModelComponent, TransformComponent};
use glam::Vec3;
use specs::{Builder, Join, WorldExt};
use uuid::Uuid;
use visible_world_action::{
    CameraUpdateData, DestroyModelData, SpawnModelData, TransformModelData, VisibleWorldAction,
    VisibleWorldActionType,
};

pub use specs;

pub mod components;
pub mod visible_world_action;

pub struct EntityBuilder<'a> {
    visible_world_actions: &'a mut Vec<VisibleWorldAction>,
    transform: Transform,
    entity_uuid: Uuid,

    builder: specs::EntityBuilder<'a>,
}

impl<'a> EntityBuilder<'a> {
    fn new(
        visible_world_actions: &'a mut Vec<VisibleWorldAction>,
        ecs: &'a mut specs::World,
        name: &str,
        transform: Transform,
    ) -> Self {
        let transform_component = TransformComponent::new(name.to_owned(), transform.clone());
        let entity_uuid = *transform_component.uuid();

        let builder = ecs.create_entity().with(transform_component);

        Self {
            visible_world_actions,
            transform,
            entity_uuid,
            builder,
        }
    }

    pub fn with<T: Component + specs::Component + Send + Sync>(self, c: T) -> Self {
        c.visible_world_actions(
            &self.transform,
            self.entity_uuid,
            self.visible_world_actions,
        );

        Self {
            visible_world_actions: self.visible_world_actions,
            transform: self.transform,
            entity_uuid: self.entity_uuid,
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
        let mut ecs = specs::World::new();
        ecs.register::<ModelComponent>();
        ecs.register::<TransformComponent>();

        Self {
            ecs,
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
            let builder = builder_pattern(EntityBuilder::new(
                &mut self.visible_world_actions,
                &mut self.ecs,
                name,
                transform,
            ));
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

    /// WARNING - This is very expensive!
    /// Add the current state of all visible elements of the world to the visible world actions.
    pub fn resync_all_visible_world_actions(&mut self) {
        appearance_profiling::profile_function!();

        self.visible_world_actions =
            vec![VisibleWorldAction::new(VisibleWorldActionType::Clear(0))];

        let (transform, model): (
            specs::ReadStorage<'_, TransformComponent>,
            specs::ReadStorage<'_, ModelComponent>,
        ) = self.ecs.system_data();

        for (transform_component, model_component) in (&transform, &model).join() {
            self.visible_world_actions.push(VisibleWorldAction::new(
                VisibleWorldActionType::SpawnModel(SpawnModelData::new(
                    transform_component.transform.get_matrix(),
                    *transform_component.uuid(),
                    &model_component.model,
                )),
            ));
        }
    }

    /// Record the final visible world actions which happened somewhere along the current frame. Call this before `get_visible_world_actions` to make sure no actions are missed.
    /// Finalization is not required when doing a resync during the same frame.
    pub fn finalize_visible_world_actions(&mut self) {
        appearance_profiling::profile_function!();

        let (transform, model): (
            specs::ReadStorage<'_, TransformComponent>,
            specs::ReadStorage<'_, ModelComponent>,
        ) = self.ecs.system_data();

        for (transform_component, _model_component) in (&transform, &model).join() {
            if transform_component.marked_for_destroy {
                self.visible_world_actions.push(VisibleWorldAction::new(
                    VisibleWorldActionType::DestroyModel(DestroyModelData {
                        entity_uuid: *transform_component.uuid(),
                    }),
                ));

                continue;
            }

            if transform_component
                .transform
                .handle_has_changed_this_frame()
            {
                self.visible_world_actions.push(VisibleWorldAction::new(
                    VisibleWorldActionType::TransformModel(TransformModelData {
                        transform_matrix: transform_component.transform.get_matrix(),
                        entity_uuid: *transform_component.uuid(),
                    }),
                ));
            }
        }
    }

    /// Receive all visible world actions which occured since the last world update.
    pub fn get_visible_world_actions(&self) -> &[VisibleWorldAction] {
        &self.visible_world_actions
    }

    /// Should be called once at the end of each frame.
    /// All visible world actions are cleared and destroyed entities are cleaned up.
    pub fn update(&mut self) {
        appearance_profiling::profile_function!();

        self.visible_world_actions.clear();

        for entity in &self.entities_marked_for_destroy {
            self.ecs.delete_entity(*entity).unwrap();
        }
        self.entities_marked_for_destroy.clear();
    }
}
