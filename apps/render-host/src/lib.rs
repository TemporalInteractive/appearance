use anyhow::Result;
use appearance::appearance_camera::CameraController;
use appearance::appearance_input::InputHandler;
use appearance::appearance_render_loop::winit::keyboard::KeyCode;
use appearance::appearance_transform::{Transform, RIGHT, UP};
use appearance::appearance_wgpu::pipeline_database::PipelineDatabase;
use appearance::appearance_wgpu::Context;
use appearance::appearance_world::components::{ModelComponent, TransformComponent};
use appearance::appearance_world::{specs, World};
use clap::Parser;
use glam::{Quat, Vec3};
use std::collections::VecDeque;
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
    /// Host port on which to listen for connections
    #[arg(long, default_value_t = 34234)]
    host_port: u16,

    /// Node port to receive events
    #[arg(long, default_value_t = 34235)]
    node_port: u16,
}

pub struct HostRenderLoop {
    pipeline_database: PipelineDatabase,
    host: Host,
    texture: wgpu::Texture,
    swapchain_format: wgpu::TextureFormat,
    timer: Timer,
    fps_history: VecDeque<f32>,

    input_handler: InputHandler,
    camera_controller: CameraController,
    world: World,

    duck_entity: Option<specs::Entity>,
    toy_car_entity: specs::Entity,
}

impl RenderLoop for HostRenderLoop {
    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_texture_dimension_1d: 4096,
            max_texture_dimension_2d: 4096,
            ..wgpu::Limits::downlevel_webgl2_defaults()
        }
    }

    fn init(config: &wgpu::SurfaceConfiguration, ctx: &Context, _window: Arc<Window>) -> Self {
        let args = Args::parse();
        let host = Host::new(args.host_port, args.node_port, config.width, config.height).unwrap();

        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let mut world = World::new();
        let duck_entity =
            world.create_entity("Duck", Transform::from_scale(Vec3::splat(1.0)), |builder| {
                builder.with(ModelComponent::new("assets/Duck.glb"))
            });
        let _ = world.create_entity(
            "Sponza",
            Transform::new(Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(1.0)),
            |builder| builder.with(ModelComponent::new("assets/Sponza.glb")),
        );
        let toy_car_entity = world.create_entity(
            "ToyCar",
            Transform::new(Vec3::new(3.0, 0.5, 0.0), Quat::IDENTITY, Vec3::splat(45.0)),
            |builder| builder.with(ModelComponent::new("assets/ToyCar.glb")),
        );
        let _ = world.create_entity(
            "Glass",
            Transform::new(
                Vec3::new(0.0, 1.0, 0.0),
                Quat::from_axis_angle(UP, 90.0f32.to_radians()),
                Vec3::splat(1.0),
            ),
            |builder| builder.with(ModelComponent::new("assets/GlassPanel.glb")),
        );

        Self {
            pipeline_database: PipelineDatabase::new(),
            host,
            texture,
            swapchain_format: config.view_formats[0],
            timer: Timer::new(),
            fps_history: VecDeque::new(),

            input_handler: InputHandler::new(),
            camera_controller: CameraController::new(),
            world,

            duck_entity: Some(duck_entity),
            toy_car_entity,
        }
    }

    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &Context) {
        self.texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
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

    fn render(&mut self, view: &wgpu::TextureView, ctx: &Context) -> bool {
        let delta_time = self.timer.elapsed();
        self.timer.reset();
        let fps = 1.0 / delta_time;

        self.fps_history.push_back(fps);
        if self.fps_history.len() > 30 {
            self.fps_history.pop_front();
        }
        let fps_avg: f32 = self.fps_history.iter().sum();
        log::info!(
            "{}ms ({} fps {} fps avg)",
            delta_time * 1000.0,
            fps,
            fps_avg / self.fps_history.len() as f32
        );

        if let Some(duck_entity) = self.duck_entity {
            let mut transforms_mut = self.world.entities_mut::<TransformComponent>();

            let duck_transform = transforms_mut.get_mut(duck_entity).unwrap();
            duck_transform.transform.translate(RIGHT * delta_time * 0.5);
        }

        {
            let mut transforms_mut = self.world.entities_mut::<TransformComponent>();
            let transform = transforms_mut.get_mut(self.toy_car_entity).unwrap();
            transform
                .transform
                .rotate(Quat::from_axis_angle(UP, delta_time * 0.3));
        }

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
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::TexelCopyBufferLayout {
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

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let texture_view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        blit_pass::encode(
            &texture_view,
            view,
            self.swapchain_format,
            &ctx.device,
            &mut command_encoder,
            &mut self.pipeline_database,
        );

        ctx.queue.submit(Some(command_encoder.finish()));

        self.input_handler.update();
        self.world.update();
        self.pipeline_database.update();

        false
    }
}

pub fn internal_main() -> Result<()> {
    let _ = Appearance::new("Render Host");
    RenderLoopHandler::<HostRenderLoop>::new(&RenderLoopWindowDesc {
        title: "Render Host".to_owned(),
        width: 1280,
        height: 640,
        resizeable: false,
        maximized: false,
    })
    .run()?;

    Ok(())
}
