use core::ops::FnMut;
use std::sync::Arc;

use appearance_path_tracer_gpu::PathTracerGpu;
use appearance_render_loop::node::NodeRenderer;
use appearance_wgpu::{pipeline_database::PipelineDatabase, wgpu, Context};
use appearance_world::visible_world_action::VisibleWorldActionType;
use futures::executor::block_on;
use glam::UVec2;

pub struct DistributedRenderer {
    ctx: Arc<Context>,
    pipeline_database: PipelineDatabase,
    path_tracer: PathTracerGpu,
}

impl Default for DistributedRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DistributedRenderer {
    pub fn new() -> Self {
        let ctx = Arc::new(block_on(Context::init(
            wgpu::Features::empty(),
            wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
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
        )));

        Self::new_with_context(ctx)
    }

    pub fn new_with_context(ctx: Arc<Context>) -> Self {
        let pipeline_database = PipelineDatabase::new();

        let path_tracer = PathTracerGpu::new(&ctx);

        Self {
            ctx,
            pipeline_database,
            path_tracer,
        }
    }
}

impl NodeRenderer for DistributedRenderer {
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
