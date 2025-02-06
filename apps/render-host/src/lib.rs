use anyhow::Result;
use appearance::appearance_camera::CameraController;
use appearance::appearance_input::InputHandler;
use appearance::appearance_render_loop::winit::keyboard::KeyCode;
use appearance::appearance_transform::{Transform, RIGHT};
use appearance::appearance_world::components::{ModelComponent, TransformComponent};
use appearance::appearance_world::{specs, World};
use clap::Parser;
use glam::{Quat, Vec3};
use std::sync::Arc;

use appearance::appearance_render_loop::host::Host;
use appearance::appearance_render_loop::winit::window::Window;
use appearance::appearance_render_loop::{
    winit, RenderLoop, RenderLoopHandler, RenderLoopWindowDesc,
};
use appearance::appearance_time::Timer;
use appearance::appearance_wgpu::helper_passes::blit_pass;
use appearance::appearance_wgpu::wgpu::{self, Extent3d, Origin3d};
use appearance::Appearance;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Host port
    #[arg(long, default_value_t = 34234)]
    host_port: u16,
}

pub struct HostRenderLoop {
    host: Host,
    texture: wgpu::Texture,
    swapchain_format: wgpu::TextureFormat,
    timer: Timer,

    input_handler: InputHandler,
    camera_controller: CameraController,
    world: World,

    duck_entity: Option<specs::Entity>,
}

impl RenderLoop for HostRenderLoop {
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_1d: 4096,
            max_texture_dimension_2d: 4096,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    }

    fn init(
        config: &wgpu::SurfaceConfiguration,
        _adapter: &wgpu::Adapter,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _window: Arc<Window>,
    ) -> Self {
        let args = Args::parse();
        let host = Host::new(args.host_port, config.width, config.height).unwrap();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let mut world = World::new();
        let duck_entity =
            world.create_entity("Duck", Transform::from_scale(Vec3::splat(1.0)), |builder| {
                builder.with(ModelComponent::new("assets/Duck.glb"))
            });
        let _ = world.create_entity(
            "Buggy",
            Transform::new(Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(0.02)),
            |builder| builder.with(ModelComponent::new("assets/Buggy.glb")),
        );

        Self {
            host,
            texture,
            swapchain_format: config.view_formats[0],
            timer: Timer::new(),

            input_handler: InputHandler::new(),
            camera_controller: CameraController::new(),
            world,

            duck_entity: Some(duck_entity),
        }
    }

    fn resize(
        &mut self,
        config: &wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) {
        self.texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
    }

    fn window_event(&mut self, event: winit::event::WindowEvent) {
        self.input_handler.handle_window_input(&event);
    }

    fn device_event(&mut self, event: winit::event::DeviceEvent) {
        self.input_handler.handle_device_input(&event);
    }

    fn render(
        &mut self,
        view: &wgpu::TextureView,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> bool {
        let delta_time = self.timer.elapsed();
        self.timer.reset();
        log::info!("{}ms ({} fps)", delta_time * 1000.0, 1.0 / delta_time);

        // if let Some(duck_entity) = self.duck_entity {
        //     let mut transforms_mut = self.world.entities_mut::<TransformComponent>();

        //     let duck_transform = transforms_mut.get_mut(duck_entity).unwrap();
        //     duck_transform.transform.translate(RIGHT * delta_time * 0.5);
        // }

        if self.input_handler.key(KeyCode::KeyX) {
            if let Some(duck_entity) = self.duck_entity.take() {
                self.world.destroy_entity(duck_entity);
            }
        }

        if self.input_handler.key(KeyCode::Escape) {
            return true;
        }

        self.world.camera_mut(|camera| {
            camera.transform =
                self.camera_controller
                    .update(camera, &self.input_handler, delta_time);
        });

        if self.host.handle_new_connections() {
            self.world.resync_all_visible_world_actions();
        } else {
            self.world.finalize_visible_world_actions();
        }

        self.host
            .send_visible_world_actions(self.world.get_visible_world_actions());

        self.host.render(|pixels| {
            queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * self.texture.width()),
                    rows_per_image: None,
                },
                Extent3d {
                    width: self.texture.width(),
                    height: self.texture.height(),
                    depth_or_array_layers: 1,
                },
            );
        });

        let mut command_encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let texture_view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        blit_pass::encode(
            &texture_view,
            view,
            self.swapchain_format,
            device,
            &mut command_encoder,
        );

        queue.submit(Some(command_encoder.finish()));

        self.input_handler.update();
        self.world.update();

        false
    }
}

pub fn internal_main() -> Result<()> {
    let _ = Appearance::new("Render Host");
    RenderLoopHandler::<HostRenderLoop>::new(&RenderLoopWindowDesc {
        title: "Render Host".to_owned(),
        width: 1024,
        height: 512,
        resizeable: false,
        maximized: false,
    })
    .run()?;

    Ok(())
}
