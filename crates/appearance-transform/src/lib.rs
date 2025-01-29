use glam::{Mat4, Quat, Vec3};
use std::sync::Mutex;

pub const RIGHT: Vec3 = Vec3::new(1.0, 0.0, 0.0);
pub const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
pub const FORWARD: Vec3 = Vec3::new(0.0, 0.0, 1.0);

#[derive(Debug)]
pub struct Transform {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
    matrix: Mutex<(Mat4, bool)>,

    has_changed_this_frame: Mutex<bool>,
}

impl Clone for Transform {
    fn clone(&self) -> Self {
        let matrix = self.matrix.lock().unwrap();
        let has_changed_this_frame = self.has_changed_this_frame.lock().unwrap();

        Self {
            translation: self.translation,
            rotation: self.rotation,
            scale: self.scale,
            matrix: Mutex::new(*matrix),
            has_changed_this_frame: Mutex::new(*has_changed_this_frame),
        }
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            matrix: Mutex::new((Mat4::IDENTITY, true)),
            has_changed_this_frame: Mutex::new(true),
        }
    }
}

impl From<Mat4> for Transform {
    fn from(value: Mat4) -> Self {
        let (scale, rotation, translation) = value.to_scale_rotation_translation();
        Self {
            translation,
            rotation,
            scale,
            ..Default::default()
        }
    }
}

impl Transform {
    pub fn new(translation: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            translation,
            rotation,
            scale,
            matrix: Mutex::new((
                Mat4::from_scale_rotation_translation(scale, rotation, translation),
                false,
            )),
            has_changed_this_frame: Mutex::new(true),
        }
    }

    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            matrix: Mutex::new((
                Mat4::from_scale_rotation_translation(Vec3::ONE, Quat::IDENTITY, translation),
                false,
            )),
            has_changed_this_frame: Mutex::new(true),
        }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale,
            matrix: Mutex::new((
                Mat4::from_scale_rotation_translation(scale, Quat::IDENTITY, Vec3::ZERO),
                false,
            )),
            has_changed_this_frame: Mutex::new(true),
        }
    }

    pub fn right(&self) -> Vec3 {
        self.rotation * RIGHT
    }

    pub fn up(&self) -> Vec3 {
        self.rotation * UP
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * FORWARD
    }

    pub fn get_translation(&self) -> Vec3 {
        self.translation
    }

    pub fn get_rotation(&self) -> Quat {
        self.rotation
    }

    pub fn get_scale(&self) -> Vec3 {
        self.scale
    }

    pub fn set_translation(&mut self, translation: Vec3) {
        self.translation = translation;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn set_scale(&mut self, scale: Vec3) {
        self.scale = scale;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn translate(&mut self, translation: Vec3) {
        self.translation += translation;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn rotate(&mut self, rotation: Quat) {
        self.rotation *= rotation;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn scale(&mut self, scale: Vec3) {
        self.scale *= scale;
        self.matrix.lock().unwrap().1 = true;
    }

    pub fn get_matrix(&self) -> Mat4 {
        let mut matrix = self.matrix.lock().unwrap();

        if matrix.1 {
            matrix.0 =
                Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
            matrix.1 = false;
        }

        matrix.0
    }

    pub fn set_matrix(&mut self, matrix: Mat4) {
        let mut my_matrix = self.matrix.lock().unwrap();
        my_matrix.0 = matrix;
        my_matrix.1 = false;

        (self.scale, self.rotation, self.translation) = matrix.to_scale_rotation_translation();
    }
}
