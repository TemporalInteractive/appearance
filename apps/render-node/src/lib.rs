use appearance::appearance_render_loop::node::{Node, NodeRenderer, NodeScissor};
use appearance::Appearance;
use glam::Vec2;

struct Renderer {
    frame_idx: u32,
}

impl Renderer {
    fn new() -> Self {
        Self { frame_idx: 0 }
    }
}

impl NodeRenderer for Renderer {
    fn render(&mut self, width: u32, height: u32, scissor: NodeScissor) -> Vec<u8> {
        log::info!("Node Render!");
        let mut pixels = vec![128; (scissor.width() * scissor.height() * 4) as usize];

        for local_x in 0..scissor.width() {
            for local_y in 0..scissor.height() {
                let x = local_x + scissor.scissor_x[0];
                let y = local_y + scissor.scissor_y[0];

                let uv = Vec2::new(x as f32 / width as f32, y as f32 / height as f32);

                pixels[(local_y * scissor.width() + local_x) as usize * 4] = (uv.x * 255.0) as u8;
                pixels[(local_y * scissor.width() + local_x) as usize * 4 + 1] =
                    (uv.y * 255.0) as u8;
                pixels[(local_y * scissor.width() + local_x) as usize * 4 + 2] = 255;
                pixels[(local_y * scissor.width() + local_x) as usize * 4 + 3] = 0;
            }
        }

        self.frame_idx += 1;

        pixels
    }
}

pub fn internal_main() {
    let _appearance = Appearance::new("Render Node");

    let node = Node::new(Renderer::new(), "127.0.0.1:34234", "127.0.0.1:34235").unwrap();
    node.run();
}
