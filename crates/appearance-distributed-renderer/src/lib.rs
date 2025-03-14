use core::ops::FnMut;
use std::sync::Arc;

use appearance_path_tracer_gpu::{PathTracerGpu, PathTracerGpuConfig};
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

impl DistributedRenderer {
    pub fn new(no_gpu_validation: bool) -> Self {
        let ctx = Arc::new(block_on(Context::init(
            wgpu::Features::empty(),
            wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::EXPERIMENTAL_RAY_QUERY
                | wgpu::Features::EXPERIMENTAL_RAY_TRACING_ACCELERATION_STRUCTURE
                | wgpu::Features::TEXTURE_BINDING_ARRAY
                | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
                | wgpu::Features::TEXTURE_COMPRESSION_BC,
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
                max_sampled_textures_per_shader_stage: 1024 * 32,
                max_binding_array_elements_per_shader_stage: 1024 * 32,
                ..wgpu::Limits::default()
            },
            no_gpu_validation,
        )));

        Self::new_with_context(ctx)
    }

    pub fn new_with_context(ctx: Arc<Context>) -> Self {
        let pipeline_database = PipelineDatabase::new();

        let path_tracer = PathTracerGpu::new(&ctx, PathTracerGpuConfig::default());

        Self {
            ctx,
            pipeline_database,
            path_tracer,
        }
    }
}

impl NodeRenderer for DistributedRenderer {
    fn visible_world_action(&mut self, action: &VisibleWorldActionType) {
        self.path_tracer
            .handle_visible_world_action(action, &self.ctx);
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
