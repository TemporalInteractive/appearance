use core::net::SocketAddr;
use core::str::FromStr;

use anyhow::Result;
use appearance::appearance_distributed_renderer::DistributedRenderer;
use appearance::appearance_render_loop::node::Node;
use appearance::Appearance;
use clap::{arg, command, Parser};

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

    let node = Node::new(DistributedRenderer::new(), addr, args.node_port)?;
    node.run();

    Ok(())
}
