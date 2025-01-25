use std::sync::Arc;

use appearance_wgpu::{wgpu, Context, Surface};
use futures::executor::block_on;
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

pub mod host;
pub mod node;

pub use winit;

pub trait RenderLoop: 'static + Sized {
    const SRGB: bool = true;

    fn optional_features() -> wgpu::Features {
        wgpu::Features::empty()
    }

    fn required_features() -> wgpu::Features {
        wgpu::Features::empty()
    }

    fn required_downlevel_capabilities() -> wgpu::DownlevelCapabilities {
        wgpu::DownlevelCapabilities {
            flags: wgpu::DownlevelFlags::empty(),
            shader_model: wgpu::ShaderModel::Sm5,
            ..wgpu::DownlevelCapabilities::default()
        }
    }

    fn required_limits() -> wgpu::Limits {
        wgpu::Limits::downlevel_webgl2_defaults()
    }

    fn init(
        config: &wgpu::SurfaceConfiguration,
        adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        window: Arc<Window>,
    ) -> Self;

    fn resize(
        &mut self,
        config: &wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    );

    fn window_event(&mut self, _event: winit::event::WindowEvent) {}
    fn device_event(&mut self, _event: winit::event::DeviceEvent) {}

    fn render(&mut self, view: &wgpu::TextureView, device: &wgpu::Device, queue: &wgpu::Queue);
}

struct RenderLoopState<R: RenderLoop> {
    window: Arc<Window>,
    surface: Surface,
    context: Context,
    render_loop: R,
}

impl<R: RenderLoop> RenderLoopState<R> {
    pub async fn from_window(mut surface: Surface, window: Arc<Window>) -> Self {
        let context = Context::init_async(
            &mut surface,
            window.clone(),
            R::optional_features(),
            R::required_features(),
            R::required_downlevel_capabilities(),
            R::required_limits(),
        )
        .await;

        surface.resume(&context, window.clone(), R::SRGB);

        let render_loop = R::init(
            surface.config(),
            &context.adapter,
            &context.device,
            &context.queue,
            window.clone(),
        );

        Self {
            window,
            surface,
            context,
            render_loop,
        }
    }
}

pub struct RenderLoopHandler<R: RenderLoop> {
    state: Option<RenderLoopState<R>>,
    frame_idx: u32,
}

impl<R: RenderLoop> Default for RenderLoopHandler<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: RenderLoop> RenderLoopHandler<R> {
    pub fn new() -> Self {
        Self {
            state: None,
            frame_idx: 0,
        }
    }

    pub fn run(mut self) -> Result<(), EventLoopError> {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(ControlFlow::Poll);
        event_loop.run_app(&mut self)
    }
}

impl<R: RenderLoop> ApplicationHandler for RenderLoopHandler<R> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let surface = if let Some(state) = self.state.take() {
            state.surface
        } else {
            Surface::new()
        };

        let window_attributes = Window::default_attributes()
            .with_title("Appearance")
            .with_maximized(true);
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

        self.state = Some(block_on(RenderLoopState::<R>::from_window(surface, window)));
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.state {
            state.surface.suspend();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(state) = &mut self.state {
            state.render_loop.window_event(event.clone());
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let frame = state.surface.acquire(&state.context);
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                        format: Some(state.surface.config().view_formats[0]),
                        ..wgpu::TextureViewDescriptor::default()
                    });
                    state
                        .render_loop
                        .render(&view, &state.context.device, &state.context.queue);

                    frame.present();

                    state.window.request_redraw();
                }

                self.frame_idx += 1;
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.surface.resize(&state.context, size);

                    state.render_loop.resize(
                        state.surface.config(),
                        &state.context.device,
                        &state.context.queue,
                    );

                    state.window.request_redraw();
                }
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let Some(state) = &mut self.state {
            state.render_loop.device_event(event.clone());
        }
    }
}
