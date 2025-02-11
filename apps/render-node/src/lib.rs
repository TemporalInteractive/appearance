use appearance::appearance_path_tracer::PathTracer;
use core::net::SocketAddr;
use core::ops::FnMut;
use core::str::FromStr;

use anyhow::Result;
use appearance::appearance_render_loop::node::{Node, NodeRenderer};
use appearance::appearance_world::visible_world_action::VisibleWorldActionType;
use appearance::Appearance;
use clap::{arg, command, Parser};

struct Renderer {
    path_tracer: PathTracer,
}

impl Renderer {
    fn new() -> Self {
        Self {
            path_tracer: PathTracer::new(),
        }
    }
}

impl NodeRenderer for Renderer {
    fn visible_world_action(&mut self, action: &VisibleWorldActionType) {
        self.path_tracer.handle_visible_world_action(action);
    }

    fn render<F: FnMut(&[u8])>(
        &mut self,
        width: u32,
        height: u32,
        start_row: u32,
        end_row: u32,
        result_callback: F,
    ) {
        self.path_tracer
            .render(width, height, start_row, end_row, result_callback);
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Ip 169.254.187.239
    #[arg(long, default_value_t = String::from("127.0.0.1"))]
    host_ip: String,

    /// Host port to connect to the host
    #[arg(long, default_value_t = 34234)]
    host_port: u16,

    /// Node port to receive events
    #[arg(long, default_value_t = 34235)]
    node_port: u16,
}

pub fn internal_main() -> Result<()> {
    let _appearance = Appearance::new("Render Node");

    let args = Args::parse();
    let addr = SocketAddr::from_str(&format!("{}:{}", args.host_ip, args.host_port)).unwrap();

    let node = Node::new(Renderer::new(), addr, args.node_port)?;
    node.run();

    Ok(())
}
