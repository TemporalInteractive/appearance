use std::sync::Mutex;

use appearance_input::InputHandler;
use appearance_transform::{Transform, RIGHT, UP};
use glam::{Mat4, Quat, Vec3};
use winit::keyboard::KeyCode;

#[derive(Debug)]
pub struct Camera {
    pub transform: Transform,
    aspect_ratio: f32,
    fov: f32,
    near: f32,
    far: f32,
    matrix: Mutex<(Mat4, bool)>,
}

#[derive(Debug)]
pub struct CameraController {
    pub translation_speed: f32,
    pub look_sensitivity: f32,
    vertical_rotation: Quat,
    horizontal_rotation: Quat,
}

impl Clone for Camera {
    fn clone(&self) -> Self {
        let matrix = self.matrix.lock().unwrap();

        Self {
            transform: self.transform.clone(),
            aspect_ratio: self.aspect_ratio,
            fov: self.fov,
            near: self.near,
            far: self.far,
            matrix: Mutex::new(*matrix),
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            transform: Transform::default(),
            aspect_ratio: 1.0,
            fov: 60.0,
            near: 0.1,
            far: 300.0,
            matrix: Mutex::new((Mat4::IDENTITY, true)),
        }
    }
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            translation_speed: 1.0,
            look_sensitivity: 0.3,
            vertical_rotation: Quat::IDENTITY,
            horizontal_rotation: Quat::IDENTITY,
        }
    }
}

impl Camera {
    pub fn new(transform: Transform, fov: f32, near: f32, far: f32, aspect_ratio: f32) -> Self {
        Self {
            transform,
            aspect_ratio,
            fov,
            near,
            far,
            ..Default::default()
        }
    }

    pub fn from_transform(transform: Transform) -> Self {
        Self {
            transform,
            ..Default::default()
        }
    }

    pub fn get_fov(&self) -> f32 {
        self.fov
    }

    pub fn set_fov(&mut self, fov: f32) {
        self.fov = fov;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn get_near(&self) -> f32 {
        self.near
    }

    pub fn set_near(&mut self, near: f32) {
        self.near = near;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn get_far(&self) -> f32 {
        self.far
    }

    pub fn set_far(&mut self, far: f32) {
        self.far = far;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn get_matrix(&self) -> Mat4 {
        let mut matrix = self.matrix.lock().unwrap();

        if matrix.1 {
            matrix.0 = Mat4::perspective_rh(
                self.fov.to_radians(),
                self.aspect_ratio,
                self.near,
                self.far,
            );
            matrix.1 = false;
        }

        matrix.0
    }
}

impl CameraController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, camera: &Camera, input: &InputHandler, delta_time: f32) -> Transform {
        let mut transform = camera.transform.clone();

        let mut velocity = Vec3::ZERO;
        if input.key(KeyCode::KeyW) {
            velocity += transform.forward();
        }
        if input.key(KeyCode::KeyS) {
            velocity -= transform.forward();
        }
        if input.key(KeyCode::KeyD) {
            velocity += transform.right();
        }
        if input.key(KeyCode::KeyA) {
            velocity -= transform.right();
        }
        if input.key(KeyCode::KeyE) {
            velocity += transform.up();
        }
        if input.key(KeyCode::KeyQ) {
            velocity -= transform.up();
        }

        self.vertical_rotation *= Quat::from_axis_angle(
            UP,
            (-input.mouse_motion().x * self.look_sensitivity).to_radians(),
        );
        self.horizontal_rotation *= Quat::from_axis_angle(
            RIGHT,
            (-input.mouse_motion().y * self.look_sensitivity).to_radians(),
        );
        transform.set_rotation(self.vertical_rotation * self.horizontal_rotation);

        if velocity.length() > 0.0 {
            let translation_speed = if input.key(KeyCode::Space) {
                self.translation_speed * 5.0
            } else if input.key(KeyCode::ControlLeft) {
                self.translation_speed * 0.2
            } else {
                self.translation_speed
            };
            transform.translate(velocity.normalize() * delta_time * translation_speed);
        }

        transform
    }
}
