use appearance::appearance_render_loop::node::{Node, NodeRenderer, NodeScissor};
use appearance::Appearance;

struct Renderer;

impl NodeRenderer for Renderer {
    fn render(&mut self, _width: u32, _height: u32, scissor: NodeScissor) -> Vec<u8> {
        vec![128; (scissor.width() * scissor.height() * 4) as usize]
    }
}

pub fn internal_main() {
    let _appearance = Appearance::new("Render Node");

    let node = Node::new(Renderer, "127.0.0.1:34234").unwrap();
    node.run();
}
