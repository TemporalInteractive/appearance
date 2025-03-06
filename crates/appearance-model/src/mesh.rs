use glam::{Vec2, Vec3, Vec4};

#[derive(Clone)]
pub struct Mesh {
    pub vertex_positions: Vec<Vec3>,
    pub vertex_normals: Vec<Vec3>,
    pub vertex_tangents: Vec<Vec4>,
    pub vertex_tex_coords: Vec<Vec2>,
    pub triangle_material_indices: Vec<u32>,
    pub indices: Vec<u32>,
    pub opaque: bool,
}

impl Mesh {
    pub fn new(
        vertex_positions: Vec<Vec3>,
        vertex_normals: Vec<Vec3>,
        vertex_tangents: Vec<Vec4>,
        vertex_tex_coords: Vec<Vec2>,
        triangle_material_indices: Vec<u32>,
        indices: Vec<u32>,
        opaque: bool,
    ) -> Self {
        debug_assert_eq!(triangle_material_indices.len(), indices.len() / 3);

        Mesh {
            vertex_positions,
            vertex_normals,
            vertex_tangents,
            vertex_tex_coords,
            triangle_material_indices,
            indices,
            opaque,
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
            let p0 = self.vertex_positions[self.indices[i * 3] as usize];
            let p1 = self.vertex_positions[self.indices[i * 3 + 1] as usize];
            let p2 = self.vertex_positions[self.indices[i * 3 + 2] as usize];
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

        if self.vertex_tex_coords.is_empty() {
            return;
        }

        self.vertex_tangents
            .resize(self.vertex_positions.len(), Vec4::ZERO);
        mikktspace::generate_tangents(self);
    }
}

impl mikktspace::Geometry for Mesh {
    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        let i = self.indices[face * 3 + vert] as usize;
        self.vertex_positions[i].into()
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        let i = self.indices[face * 3 + vert] as usize;
        self.vertex_normals[i].into()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        let i = self.indices[face * 3 + vert] as usize;
        self.vertex_tex_coords[i].into()
    }

    fn set_tangent(
        &mut self,
        tangent: [f32; 3],
        _bi_tangent: [f32; 3],
        _f_mag_s: f32,
        _f_mag_t: f32,
        bi_tangent_preserves_orientation: bool,
        face: usize,
        vert: usize,
    ) {
        let sign = if bi_tangent_preserves_orientation {
            1.0
        } else {
            -1.0
        };

        let i = self.indices[face * 3 + vert] as usize;
        self.vertex_tangents[i] = Vec4::new(tangent[0], tangent[1], tangent[2], sign);
    }
}
