use anyhow::Result;
use core::str;
use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use appearance_asset_database::{Asset, AssetDatabase};

pub struct ShaderAsset {
    pub file_path: String,
    pub src: String,
    shader_module: OnceLock<wgpu::ShaderModule>,
}

impl Asset for ShaderAsset {
    fn load(file_path: &str, data: &[u8]) -> Result<Self> {
        let src = str::from_utf8(data)?.to_owned();

        Ok(Self {
            file_path: file_path.to_owned(),
            src,
            shader_module: OnceLock::new(),
        })
    }
}

impl ShaderAsset {
    pub fn shader_module(&self, device: &wgpu::Device) -> &wgpu::ShaderModule {
        self.shader_module.get_or_init(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&self.file_path),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(self.src.as_str())),
            })
        })
    }
}

#[macro_export]
macro_rules! include_shader_src {
    ($NAME:literal) => {
        include_str!(concat!(
            concat!(env!("OUT_DIR"), "/../../../assets/"),
            $NAME
        ))
    };
}

pub struct PipelineDatabase {
    asset_shader_modules: AssetDatabase<ShaderAsset>,
    shader_modules: HashMap<String, Arc<wgpu::ShaderModule>>,
    render_pipelines: HashMap<String, Arc<wgpu::RenderPipeline>>,
    compute_pipelines: HashMap<String, Arc<wgpu::ComputePipeline>>,
}

impl Default for PipelineDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineDatabase {
    pub fn new() -> Self {
        Self {
            asset_shader_modules: AssetDatabase::new(),
            shader_modules: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
        }
    }

    pub fn update(&mut self) {
        appearance_profiling::profile_function!();

        self.asset_shader_modules.update();
    }

    pub fn shader(&mut self, path: &str) -> Result<Arc<ShaderAsset>> {
        appearance_profiling::profile_function!();

        self.asset_shader_modules.get(path)
    }

    pub fn shader_from_src(&mut self, device: &wgpu::Device, src: &str) -> Arc<wgpu::ShaderModule> {
        appearance_profiling::profile_function!();

        if let Some(module) = self.shader_modules.get(src) {
            return module.clone();
        }

        let module = Arc::new(device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(src),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(src)),
        }));

        self.shader_modules.insert(src.to_owned(), module.clone());
        module
    }

    pub fn render_pipeline<F>(
        &mut self,
        device: &wgpu::Device,
        descriptor: wgpu::RenderPipelineDescriptor,
        create_layout_fn: F,
    ) -> Arc<wgpu::RenderPipeline>
    where
        F: Fn() -> wgpu::PipelineLayout, //d
    {
        appearance_profiling::profile_function!();

        let entry = descriptor
            .label
            .expect("Every pipeline must contain a label!");
        if let Some(pipeline) = self.render_pipelines.get(entry) {
            return pipeline.clone();
        }

        let pipeline_layout = create_layout_fn();
        let descriptor = wgpu::RenderPipelineDescriptor {
            layout: Some(&pipeline_layout),
            ..descriptor
        };

        let pipeline = Arc::new(device.create_render_pipeline(&descriptor));

        self.render_pipelines
            .insert(entry.to_owned(), pipeline.clone());
        pipeline
    }

    pub fn compute_pipeline<F>(
        &mut self,
        device: &wgpu::Device,
        descriptor: wgpu::ComputePipelineDescriptor,
        create_layout_fn: F,
    ) -> Arc<wgpu::ComputePipeline>
    where
        F: Fn() -> wgpu::PipelineLayout,
    {
        appearance_profiling::profile_function!();

        let entry = descriptor
            .label
            .expect("Every pipeline must contain a label!");
        if let Some(pipeline) = self.compute_pipelines.get(entry) {
            return pipeline.clone();
        }

        let pipeline_layout = create_layout_fn();
        let descriptor = wgpu::ComputePipelineDescriptor {
            layout: Some(&pipeline_layout),
            ..descriptor
        };

        let pipeline = Arc::new(device.create_compute_pipeline(&descriptor));

        self.compute_pipelines
            .insert(entry.to_owned(), pipeline.clone());
        pipeline
    }
}
