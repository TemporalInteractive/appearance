use glam::{Vec2, Vec3, Vec4, Vec4Swizzles};
use std::rc::Rc;

use tinybvh::Bvh;

pub struct Mesh {
    pub vertex_positions: Vec<Vec4>,
    pub vertex_normals: Vec<Vec3>,
    pub vertex_tangents: Vec<Vec4>,
    pub vertex_tex_coords: Vec<Vec2>,
    pub indices: Vec<u32>,
    pub material_idx: u32,

    pub blas: Rc<Bvh>,
}

impl Mesh {
    pub fn new(
        vertex_positions: Vec<Vec4>,
        vertex_normals: Vec<Vec3>,
        vertex_tangents: Vec<Vec4>,
        vertex_tex_coords: Vec<Vec2>,
        indices: Vec<u32>,
        material_idx: u32,
    ) -> Self {
        let mut blas = Bvh::new();
        if indices.is_empty() {
            blas.build(vertex_positions.clone());
        } else {
            blas.build_with_indices(vertex_positions.clone(), indices.clone());
        }

        Mesh {
            vertex_positions,
            vertex_normals,
            vertex_tangents,
            vertex_tex_coords,
            indices,
            material_idx,
            blas: Rc::new(blas),
        }
    }

    pub fn has_normals(&self) -> bool {
        !self.vertex_normals.is_empty()
    }

    pub fn has_tangents(&self) -> bool {
        !self.vertex_tangents.is_empty()
    }

    pub fn generate_normals(&mut self) {
        appearance_profiling::profile_function!();

        for normal in &mut self.vertex_normals {
            *normal = Vec3::ZERO;
        }
        self.vertex_normals
            .resize(self.vertex_positions.len(), Vec3::ZERO);

        for i in 0..(self.indices.len() / 3) {
            let p0 = self.vertex_positions[self.indices[i * 3] as usize].xyz();
            let p1 = self.vertex_positions[self.indices[i * 3 + 1] as usize].xyz();
            let p2 = self.vertex_positions[self.indices[i * 3 + 2] as usize].xyz();
            let n = (p1 - p0).cross(p2 - p0).normalize();

            self.vertex_normals[self.indices[i * 3] as usize] += n;
            self.vertex_normals[self.indices[i * 3 + 1] as usize] += n;
            self.vertex_normals[self.indices[i * 3 + 2] as usize] += n;
        }

        for normal in &mut self.vertex_normals {
            *normal = normal.normalize();
        }
    }

    pub fn generate_tangents(&mut self) {
        appearance_profiling::profile_function!();

        // Source: 2001. http://www.terathon.com/code/tangent.html
        let mut tan1 = vec![Vec3::default(); self.vertex_positions.len()];
        let mut tan2 = vec![Vec3::default(); self.vertex_positions.len()];

        for i in (0..self.indices.len()).step_by(3) {
            let i1 = self.indices[i] as usize;
            let i2 = self.indices[i + 1] as usize;
            let i3 = self.indices[i + 2] as usize;

            let v1 = self.vertex_positions[i1].xyz();
            let v2 = self.vertex_positions[i2].xyz();
            let v3 = self.vertex_positions[i3].xyz();

            let w1 = self.vertex_tex_coords[i1];
            let w2 = self.vertex_tex_coords[i2];
            let w3 = self.vertex_tex_coords[i3];

            let x1 = v2.x - v1.x;
            let x2 = v3.x - v1.x;
            let y1 = v2.y - v1.y;
            let y2 = v3.y - v1.y;
            let z1 = v2.z - v1.z;
            let z2 = v3.z - v1.z;

            let s1 = w2.x - w1.x;
            let s2 = w3.x - w1.x;
            let t1 = w2.y - w1.y;
            let t2 = w3.y - w1.y;

            let rdiv = s1 * t2 - s2 * t1;
            let r = if rdiv == 0.0 { 0.0 } else { 1.0 / rdiv };

            let sdir = Vec3::new(
                (t2 * x1 - t1 * x2) * r,
                (t2 * y1 - t1 * y2) * r,
                (t2 * z1 - t1 * z2) * r,
            );

            let tdir = Vec3::new(
                (s1 * x2 - s2 * x1) * r,
                (s1 * y2 - s2 * y1) * r,
                (s1 * z2 - s2 * z1) * r,
            );

            tan1[i1] += sdir;
            tan1[i2] += sdir;
            tan1[i3] += sdir;

            tan2[i1] += tdir;
            tan2[i2] += tdir;
            tan2[i3] += tdir;
        }

        self.vertex_tangents
            .resize(self.vertex_positions.len(), Vec4::ZERO);

        for i in 0..self.vertex_positions.len() {
            let n = self.vertex_normals[i];
            let t = tan1[i];

            let xyz = (t - (n * n.dot(t))).normalize();

            let w = if n.cross(t).dot(tan2[i]) < 0.0 {
                -1.0
            } else {
                1.0
            };

            self.vertex_tangents[i] = Vec4::new(xyz.x, xyz.y, xyz.z, w);
        }
    }
}
