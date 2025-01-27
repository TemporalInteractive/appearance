use anyhow::Result;
use appearance::appearance_render_loop::node::{Node, NodeRenderer};
use appearance::Appearance;
use glam::{Vec2, Vec3};

struct Renderer {
    pixels: Vec<u8>,
    frame_idx: u32,
}

impl Renderer {
    fn new() -> Self {
        Self {
            pixels: Vec::new(),
            frame_idx: 0,
        }
    }

    fn render_pixel(uv: &Vec2) -> Vec3 {
        Vec3::new(uv.x, uv.y, 1.0)
    }
}

impl NodeRenderer for Renderer {
    fn render(&mut self, width: u32, height: u32, assigned_rows: [u32; 2]) -> &[u8] {
        let start_row = assigned_rows[0];
        let end_row = assigned_rows[1];
        let num_rows = end_row - start_row;

        self.pixels.resize((width * num_rows * 4) as usize, 0);

        for local_x in 0..width {
            for local_y in 0..num_rows {
                let x = local_x;
                let y = local_y + start_row;
                let uv = Vec2::new(x as f32 / width as f32, y as f32 / height as f32);

                let result = Self::render_pixel(&uv);

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
