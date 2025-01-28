use anyhow::Result;
use appearance::appearance_camera::Camera;
use appearance::appearance_render_loop::node::{Node, NodeRenderer};
use appearance::appearance_world::visible_world_action::VisibleWorldActionType;
use appearance::Appearance;
use glam::{Mat4, Vec2, Vec3, Vec4, Vec4Swizzles};
use tinybvh::Bvh;
use tinybvh::{vec_helpers::Vec3Helpers, Ray};

struct CameraMatrices {
    inv_view: Mat4,
    inv_proj: Mat4,
}

struct Renderer {
    pixels: Vec<u8>,
    frame_idx: u32,

    camera: Camera,
    blas: Bvh,
}

const VERTICES: &[Vec4] = &[
    Vec4::new(-1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(1.0, 0.0, 0.0, 0.0),
];

impl Renderer {
    fn new() -> Self {
        let mut blas = Bvh::new();
        blas.build(VERTICES);

        Self {
            pixels: Vec::new(),
            frame_idx: 0,
            camera: Camera::default(),
            blas,
        }
    }

    fn render_pixel(&mut self, uv: &Vec2, camera_matrices: &CameraMatrices) -> Vec3 {
        let corrected_uv = Vec2::new(uv.x, -uv.y);
        let origin = camera_matrices.inv_view * Vec4::new(0.0, 0.0, 0.0, 1.0);
        let target = camera_matrices.inv_proj * Vec4::from((corrected_uv, 1.0, 1.0));
        let direction = camera_matrices.inv_view * Vec4::from((target.xyz().normalize(), 0.0));

        let mut ray = Ray::new(origin.xyz(), direction.xyz());

        self.blas.intersect(&mut ray);
        if ray.hit.t != 1e30 {
            Vec3::new(0.0, 1.0, 1.0)
        } else {
            let a = 0.5 * (ray.D.y() + 1.0);
            (1.0 - a) * Vec3::new(1.0, 1.0, 1.0) + a * Vec3::new(0.5, 0.7, 1.0)
        }
    }
}

impl NodeRenderer for Renderer {
    fn visible_world_action(&mut self, action: &VisibleWorldActionType) {
        match action {
            VisibleWorldActionType::CameraUpdate(data) => {
                self.camera.set_near(data.near);
                self.camera.set_far(data.far);
                self.camera.set_fov(data.fov);
                self.camera
                    .transform
                    .set_matrix(data.transform_matrix_bytes);
            }
        }
    }

    fn render(&mut self, width: u32, height: u32, assigned_rows: [u32; 2]) -> &[u8] {
        let start_row = assigned_rows[0];
        let end_row = assigned_rows[1];
        let num_rows = end_row - start_row;
        self.pixels.resize((width * num_rows * 4) as usize, 0);

        self.camera.set_aspect_ratio(width as f32 / height as f32);

        let camera_matrices = CameraMatrices {
            inv_view: self.camera.transform.get_matrix(),
            inv_proj: self.camera.get_matrix().inverse(),
        };

        for local_y in 0..num_rows {
            for local_x in 0..width {
                let x = local_x;
                let y = local_y + start_row;
                let uv = Vec2::new(
                    (x as f32 + 0.5) / width as f32,
                    (y as f32 + 0.5) / height as f32,
                ) * 2.0
                    - 1.0;

                let result = self.render_pixel(&uv, &camera_matrices);

                self.pixels[(local_y * width + local_x) as usize * 4] = (result.x * 255.0) as u8;
                self.pixels[(local_y * width + local_x) as usize * 4 + 1] =
                    (result.y * 255.0) as u8;
                self.pixels[(local_y * width + local_x) as usize * 4 + 2] =
                    (result.z * 255.0) as u8;
                self.pixels[(local_y * width + local_x) as usize * 4 + 3] = 255;
            }
        }

        self.frame_idx += 1;

        &self.pixels
    }
}

pub fn internal_main() -> Result<()> {
    let _appearance = Appearance::new("Render Node");

    let node = Node::new(Renderer::new(), "127.0.0.1:34234")?;
    node.run();

    Ok(())
}
