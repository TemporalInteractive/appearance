use bytemuck::Pod;
use futures::{channel::oneshot, executor::block_on};
use std::{future::IntoFuture, sync::Arc};
use wgpu::{DownlevelCapabilities, Features, Instance, Limits, PowerPreference};
use winit::{
    dpi::PhysicalSize,
    event::{Event, StartCause},
    window::Window,
};

pub mod helper_passes;
pub mod pipeline_database;

pub use wgpu;

#[derive(Default)]
pub struct Surface {
    surface: Option<wgpu::Surface<'static>>,
    config: Option<wgpu::SurfaceConfiguration>,
}

impl Surface {
    /// Create a new surface wrapper with no surface or configuration.
    pub fn new() -> Self {
        Self {
            surface: None,
            config: None,
        }
    }

    /// Called after the instance is created, but before we request an adapter.
    ///
    /// On wasm, we need to create the surface here, as the WebGL backend needs
    /// a surface (and hence a canvas) to be present to create the adapter.
    ///
    /// We cannot unconditionally create a surface here, as Android requires
    /// us to wait until we receive the `Resumed` event to do so.
    pub fn pre_adapter(&mut self, instance: &Instance, window: Arc<Window>) {
        if cfg!(target_arch = "wasm32") {
            self.surface = Some(instance.create_surface(window).unwrap());
        }
    }

    /// Check if the event is the start condition for the surface.
    pub fn start_condition(e: &Event<()>) -> bool {
        match e {
            // On all other platforms, we can create the surface immediately.
            Event::NewEvents(StartCause::Init) => !cfg!(target_os = "android"),
            // On android we need to wait for a resumed event to create the surface.
            Event::Resumed => cfg!(target_os = "android"),
            _ => false,
        }
    }

    /// Called when an event which matches [`Self::start_condition`] is received.
    ///
    /// On all native platforms, this is where we create the surface.
    ///
    /// Additionally, we configure the surface based on the (now valid) window size.
    pub fn resume(&mut self, context: &Context, window: Arc<Window>, srgb: bool) {
        // Window size is only actually valid after we enter the event loop.
        let window_size = window.inner_size();
        let width = window_size.width.max(1);
        let height = window_size.height.max(1);

        log::info!("Surface resume {window_size:?}");

        // We didn't create the surface in pre_adapter, so we need to do so now.
        if !cfg!(target_arch = "wasm32") {
            self.surface = Some(context.instance.create_surface(window).unwrap());
        }

        // From here on, self.surface should be Some.

        let surface = self.surface.as_ref().unwrap();

        // Get the default configuration,
        let mut config = surface
            .get_default_config(&context.adapter, width, height)
            .expect("Surface isn't supported by the adapter.");
        if srgb {
            // Not all platforms (WebGPU) support sRGB swapchains, so we need to use view formats
            let view_format = config.format.add_srgb_suffix();
            config.view_formats.push(view_format);
        } else {
            // All platforms support non-sRGB swapchains, so we can just use the format directly.
            let format = config.format.remove_srgb_suffix();
            config.format = format;
            config.view_formats.push(format);
        };
        config.present_mode = wgpu::PresentMode::AutoNoVsync;

        surface.configure(&context.device, &config);
        self.config = Some(config);
    }

    /// Resize the surface, making sure to not resize to zero.
    pub fn resize(&mut self, context: &Context, size: PhysicalSize<u32>) {
        log::info!("Surface resize {size:?}");

        let config = self.config.as_mut().unwrap();
        config.width = size.width.max(1);
        config.height = size.height.max(1);
        let surface = self.surface.as_ref().unwrap();
        surface.configure(&context.device, config);
    }

    /// Acquire the next surface texture.
    pub fn acquire(&mut self, context: &Context) -> wgpu::SurfaceTexture {
        let surface = self.surface.as_ref().unwrap();

        match surface.get_current_texture() {
            Ok(frame) => frame,
            // If we timed out, just try again
            Err(wgpu::SurfaceError::Timeout) => surface
                .get_current_texture()
                .expect("Failed to acquire next surface texture!"),
            Err(
                // If the surface is outdated, or was lost, reconfigure it.
                wgpu::SurfaceError::Outdated
                | wgpu::SurfaceError::Lost
                // If OutOfMemory happens, reconfiguring may not help, but we might as well try
                | wgpu::SurfaceError::OutOfMemory | wgpu::SurfaceError::Other,
            ) => {
                surface.configure(&context.device, self.config());
                surface
                    .get_current_texture()
                    .expect("Failed to acquire next surface texture!")
            }
        }
    }

    /// On suspend on android, we drop the surface, as it's no longer valid.
    ///
    /// A suspend event is always followed by at least one resume event.
    pub fn suspend(&mut self) {
        if cfg!(target_os = "android") {
            self.surface = None;
        }
    }

    pub fn get(&self) -> Option<&wgpu::Surface> {
        self.surface.as_ref()
    }

    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        self.config.as_ref().unwrap()
    }
}

pub struct Context {
    instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Context {
    async fn init_with_instance(
        instance: Instance,
        optional_features: Features,
        required_features: Features,
        required_downlevel_capabilities: DownlevelCapabilities,
        required_limits: Limits,
    ) -> Self {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to find suitable GPU adapter.");

        let adapter_features = adapter.features();
        assert!(
            adapter_features.contains(required_features),
            "Adapter does not support required features for this example: {:?}",
            required_features - adapter_features
        );

        let downlevel_capabilities = adapter.get_downlevel_capabilities();
        assert!(
            downlevel_capabilities.shader_model >= required_downlevel_capabilities.shader_model,
            "Adapter does not support the minimum shader model required to run this example: {:?}",
            required_downlevel_capabilities.shader_model
        );
        assert!(
            downlevel_capabilities
                .flags
                .contains(required_downlevel_capabilities.flags),
            "Adapter does not support the downlevel capabilities required to run this example: {:?}",
            required_downlevel_capabilities.flags - downlevel_capabilities.flags
        );

        let trace_dir = std::env::var("WGPU_TRACE");
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: (optional_features & adapter_features) | required_features,
                    required_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                trace_dir.ok().as_ref().map(std::path::Path::new),
            )
            .await
            .expect("Unable to find a suitable GPU adapter!");

        Self {
            instance,
            adapter,
            device,
            queue,
        }
    }

    pub async fn init_with_window(
        surface: &mut Surface,
        window: Arc<Window>,
        optional_features: Features,
        required_features: Features,
        required_downlevel_capabilities: DownlevelCapabilities,
        required_limits: Limits,
        no_gpu_validation: bool,
    ) -> Self {
        log::info!("Initializing wgpu...");

        let mut flags = wgpu::InstanceFlags::DEBUG;
        if !no_gpu_validation {
            flags |= wgpu::InstanceFlags::VALIDATION;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags,
            backend_options: wgpu::BackendOptions::default(),
        });
        surface.pre_adapter(&instance, window);

        Self::init_with_instance(
            instance,
            optional_features,
            required_features,
            required_downlevel_capabilities,
            required_limits,
        )
        .await
    }

    pub async fn init(
        optional_features: Features,
        required_features: Features,
        required_downlevel_capabilities: DownlevelCapabilities,
        required_limits: Limits,
        no_gpu_validation: bool,
    ) -> Self {
        log::info!("Initializing wgpu...");

        let mut flags = wgpu::InstanceFlags::DEBUG;
        if !no_gpu_validation {
            flags |= wgpu::InstanceFlags::VALIDATION;
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags,
            backend_options: wgpu::BackendOptions::default(),
        });

        Self::init_with_instance(
            instance,
            optional_features,
            required_features,
            required_downlevel_capabilities,
            required_limits,
        )
        .await
    }
}

pub trait ComputePipelineDescriptorExtensions<'a> {
    fn partial_default(module: &'a wgpu::ShaderModule) -> Self;
}

impl<'a> ComputePipelineDescriptorExtensions<'a> for wgpu::ComputePipelineDescriptor<'a> {
    fn partial_default(module: &'a wgpu::ShaderModule) -> Self {
        wgpu::ComputePipelineDescriptor {
            label: None,
            layout: None,
            module,
            entry_point: None,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        }
    }
}

pub async fn readback_buffer_async<T: Pod>(
    staging_buffer: &wgpu::Buffer,
    device: &wgpu::Device,
) -> Vec<T> {
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    device.poll(wgpu::PollType::Wait).unwrap();
    receiver.into_future().await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let result = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging_buffer.unmap();
    result
}

pub fn readback_buffer<T: Pod>(staging_buffer: &wgpu::Buffer, device: &wgpu::Device) -> Vec<T> {
    block_on(readback_buffer_async(staging_buffer, device))
}

static EMPTY_TEXTURE_VIEW: std::sync::OnceLock<wgpu::TextureView> = std::sync::OnceLock::new();

pub fn empty_texture_view(device: &wgpu::Device) -> &wgpu::TextureView {
    EMPTY_TEXTURE_VIEW.get_or_init(|| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            label: Some("Empty"),
            view_formats: &[],
        });
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        })
    })
}

static EMPTY_BIND_GROUP_LAYOUT: std::sync::OnceLock<wgpu::BindGroupLayout> =
    std::sync::OnceLock::new();

pub fn empty_bind_group_layout(device: &wgpu::Device) -> &wgpu::BindGroupLayout {
    EMPTY_BIND_GROUP_LAYOUT.get_or_init(|| {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[],
        })
    })
}

static EMPTY_BIND_GROUP: std::sync::OnceLock<wgpu::BindGroup> = std::sync::OnceLock::new();

pub fn empty_bind_group(device: &wgpu::Device) -> &wgpu::BindGroup {
    EMPTY_BIND_GROUP.get_or_init(|| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: empty_bind_group_layout(device),
            entries: &[],
        })
    })
}
