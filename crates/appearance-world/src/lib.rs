use appearance_camera::Camera;
use appearance_transform::Transform;
use glam::Vec3;
use visible_world_action::{CameraUpdateData, VisibleWorldAction, VisibleWorldActionType};

pub mod visible_world_action;

/// The world is how the game is percieved by the host, including not only visual but also gameplay elements
pub struct World {
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
        self.visible_world_actions.clear();
    }
}
