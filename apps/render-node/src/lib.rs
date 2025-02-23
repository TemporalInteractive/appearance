use core::net::SocketAddr;
use core::ops::FnMut;
use core::str::FromStr;

use anyhow::Result;
use appearance::appearance_path_tracer_gpu::PathTracerGpu;
use appearance::appearance_render_loop::node::{Node, NodeRenderer};
use appearance::appearance_wgpu::pipeline_database::PipelineDatabase;
use appearance::appearance_wgpu::{wgpu, Context};
use appearance::appearance_world::visible_world_action::VisibleWorldActionType;
use appearance::Appearance;
use clap::{arg, command, Parser};
use futures::executor::block_on;
use glam::UVec2;

struct Renderer {
    ctx: Context,
    pipeline_database: PipelineDatabase,
    path_tracer: PathTracerGpu,
}

impl Renderer {
    fn new() -> Self {
        let ctx = block_on(Context::init(
            wgpu::Features::empty(),
            wgpu::Features::empty(),
            wgpu::DownlevelCapabilities {
                flags: wgpu::DownlevelFlags::empty(),
                shader_model: wgpu::ShaderModel::Sm5,
                ..wgpu::DownlevelCapabilities::default()
            },
            wgpu::Limits {
                max_compute_invocations_per_workgroup: 512,
                max_compute_workgroup_size_x: 512,
                max_buffer_size: (1024 << 20),
                max_storage_buffer_binding_size: (1024 << 20),
                ..wgpu::Limits::default()
            },
        ));

        let pipeline_database = PipelineDatabase::new();

        let path_tracer = PathTracerGpu::new(&ctx);

        Self {
            ctx,
            pipeline_database,
            path_tracer,
        }
    }
}

impl NodeRenderer for Renderer {
    fn visible_world_action(&mut self, action: &VisibleWorldActionType) {
        self.path_tracer.handle_visible_world_action(action);
    }

    fn render<F: FnMut(&[u8])>(
        &mut self,
        resolution: UVec2,
        start_row: u32,
        end_row: u32,
        result_callback: F,
    ) {
        self.path_tracer.render(
            resolution,
            start_row,
            end_row,
            result_callback,
            &self.ctx,
            &mut self.pipeline_database,
        );
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
