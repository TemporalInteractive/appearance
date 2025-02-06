use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Normal(Vec3);

impl From<Vec3> for Normal {
    fn from(v: Vec3) -> Self {
        Normal(v)
    }
}

impl From<Normal> for Vec3 {
    fn from(val: Normal) -> Self {
        val.0
    }
}

impl Normal {
    pub fn new(v: Vec3) -> Self {
        Self(v)
    }

    pub fn forward_facing(&self, forward: &Vec3) -> Normal {
        if self.0.dot(*forward) < 0.0 {
            Normal::new(-self.0)
        } else {
            *self
        }
    }
}
