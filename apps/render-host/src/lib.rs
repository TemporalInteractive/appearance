use anyhow::Result;
use appearance::appearance_camera::CameraController;
use appearance::appearance_distributed_renderer::DistributedRenderer;
use appearance::appearance_input::InputHandler;
use appearance::appearance_render_loop::block_to_linear_pass::BlockToLinearPassParameters;
use appearance::appearance_render_loop::node::NodeRenderer;
use appearance::appearance_render_loop::winit::keyboard::KeyCode;
use appearance::appearance_transform::{Transform, RIGHT, UP};
use appearance::appearance_wgpu::pipeline_database::PipelineDatabase;
use appearance::appearance_wgpu::Context;
use appearance::appearance_world::components::{ModelComponent, TransformComponent};
use appearance::appearance_world::visible_world_action::VisibleWorldActionType;
use appearance::appearance_world::{specs, World};
use clap::Parser;
use glam::{Quat, UVec2, Vec3};
use std::collections::VecDeque;
use std::sync::Arc;

use appearance::appearance_render_loop::host::{Host, RENDER_BLOCK_SIZE};
use appearance::appearance_render_loop::winit::window::Window;
use appearance::appearance_render_loop::{
    block_to_linear_pass, winit, RenderLoop, RenderLoopHandler, RenderLoopWindowDesc,
};
use appearance::appearance_time::Timer;
use appearance::appearance_wgpu::helper_passes::blit_pass;
use appearance::appearance_wgpu::wgpu::{self, Extent3d, Origin3d};
use appearance::Appearance;

enum RenderingStrategy {
    Distributed(Host),
    Local(DistributedRenderer),
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Host port on which to listen for connections
    #[arg(long, default_value_t = 34234)]
    host_port: u16,

    /// Node port to receive events
    #[arg(long, default_value_t = 34235)]
    node_port: u16,

    /// Run the renderer inside the host as a traditional engine would
    #[arg(long, default_value_t = true)]
    render_local: bool,

    /// Forcefully disable gpu validation
    #[arg(long, default_value_t = false)]
    no_gpu_validation: bool,
}

pub struct HostRenderLoop {
    pipeline_database: PipelineDatabase,
    rendering_strategy: RenderingStrategy,
    texture: [wgpu::Texture; 2],
    swapchain_format: wgpu::TextureFormat,
    timer: Timer,
    fps_history: VecDeque<f32>,

    input_handler: InputHandler,
    camera_controller: CameraController,
    world: World,
    // duck_entity: Option<specs::Entity>,
    // toy_car_entity: specs::Entity,
}

impl RenderLoop for HostRenderLoop {
    fn required_features() -> wgpu::Features {
        wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
            | wgpu::Features::EXPERIMENTAL_RAY_QUERY
            | wgpu::Features::EXPERIMENTAL_RAY_TRACING_ACCELERATION_STRUCTURE
            | wgpu::Features::TEXTURE_BINDING_ARRAY
            | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
            | wgpu::Features::TEXTURE_COMPRESSION_BC
            | wgpu::Features::PUSH_CONSTANTS
    }

    fn required_limits() -> wgpu::Limits {
        wgpu::Limits {
            max_compute_invocations_per_workgroup: 512,
            max_compute_workgroup_size_x: 512,
            max_buffer_size: (1024 << 20),
            max_storage_buffer_binding_size: (1024 << 20),
            max_sampled_textures_per_shader_stage: 1024 * 32,
            max_binding_array_elements_per_shader_stage: 1024 * 32,
            max_push_constant_size: 128,
            max_bind_groups: 8,
            ..wgpu::Limits::default()
        }
    }

    fn init(config: &wgpu::SurfaceConfiguration, ctx: &Arc<Context>, _window: Arc<Window>) -> Self {
        let args = Args::parse();

        let rendering_strategy = if args.render_local {
            let distributed_renderer = DistributedRenderer::new_with_context(ctx.clone());
            RenderingStrategy::Local(distributed_renderer)
        } else {
            let host =
                Host::new(args.host_port, args.node_port, config.width, config.height).unwrap();
            RenderingStrategy::Distributed(host)
        };

        let texture = std::array::from_fn(|_| {
            ctx.device.create_texture(&wgpu::TextureDescriptor {
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
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        });

        let mut world = World::new();
        // let duck_entity =
        //     world.create_entity("Duck", Transform::from_scale(Vec3::splat(1.0)), |builder| {
        //         builder.with(ModelComponent::new("::Duck.glb"))
        //     });
        let _ = world.create_entity(
            "Sponza",
            Transform::new(Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(1.0)),
            |builder| builder.with(ModelComponent::new("::SponzaGlossy.glb")),
        );
        // let _ = world.create_entity(
        //     "CornellBox",
        //     Transform::new(Vec3::new(0.0, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(1.0)),
        //     |builder| builder.with(ModelComponent::new("::CornellBoxConor.glb")),
        // );
        let _ = world.create_entity(
            "Orbs",
            Transform::new(Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(1.0)),
            |builder| builder.with(ModelComponent::new("::SponzaOrbs.glb")),
        );
        // let _ = world.create_entity(
        //     "NeonSigns",
        //     Transform::new(Vec3::new(3.0, 0.0, 0.0), Quat::IDENTITY, Vec3::splat(1.0)),
        //     |builder| builder.with(ModelComponent::new("::SponzaNeon.glb")),
        // );
        // let toy_car_entity = world.create_entity(
        //     "ToyCar",
        //     Transform::new(Vec3::new(3.0, 0.5, 0.0), Quat::IDENTITY, Vec3::splat(45.0)),
        //     |builder| builder.with(ModelComponent::new("::ToyCarNonEmissive.glb")),
        // );
        // let _ = world.create_entity(
        //     "Chess",
        //     Transform::new(Vec3::new(-2.0, 0.5, 0.0), Quat::IDENTITY, Vec3::splat(3.0)),
        //     |builder| builder.with(ModelComponent::new("::ABeautifulGame.glb")),
        // );
        // let _ = world.create_entity(
        //     "Glass",
        //     Transform::new(
        //         Vec3::new(0.0, 1.0, 0.0),
        //         Quat::from_axis_angle(UP, 90.0f32.to_radians()),
        //         Vec3::splat(1.0),
        //     ),
        //     |builder| builder.with(ModelComponent::new("::GlassPanel.glb")),
        // );
        // let _ = world.create_entity(
        //     "Dragon",
        //     Transform::new(
        //         Vec3::new(-4.0, 0.5, 0.0),
        //         Quat::from_axis_angle(UP, 90.0f32.to_radians()),
        //         Vec3::splat(0.5),
        //     ),
        //     |builder| builder.with(ModelComponent::new("::DragonAttenuation.glb")),
        // );

        // let _ = world.create_entity(
        //     "ClearCoatTest",
        //     Transform::new(
        //         Vec3::new(0.0, 0.0, 15.0),
        //         Quat::from_axis_angle(UP, 90.0f32.to_radians()),
        //         Vec3::splat(1.0),
        //     ),
        //     |builder| builder.with(ModelComponent::new("::test_models/ClearCoatTest.glb")),
        // );

        // let _ = world.create_entity(
        //     "AttenuationTest",
        //     Transform::new(
        //         Vec3::new(0.0, 0.0, 35.0),
        //         Quat::from_axis_angle(UP, 90.0f32.to_radians()),
        //         Vec3::splat(1.0),
        //     ),
        //     |builder| builder.with(ModelComponent::new("::test_models/AttenuationTest.glb")),
        // );

        Self {
            pipeline_database: PipelineDatabase::new(),
            rendering_strategy,
            texture,
            swapchain_format: config.view_formats[0],
            timer: Timer::new(),
            fps_history: VecDeque::new(),

            input_handler: InputHandler::new(),
            camera_controller: CameraController::new(),
            world,
            // duck_entity: Some(duck_entity),
            // toy_car_entity,
        }
    }

    fn resize(&mut self, config: &wgpu::SurfaceConfiguration, ctx: &Context) {
        let width = config.width.div_ceil(RENDER_BLOCK_SIZE) * RENDER_BLOCK_SIZE;
        let height = config.height.div_ceil(RENDER_BLOCK_SIZE) * RENDER_BLOCK_SIZE;

        self.texture = std::array::from_fn(|_| {
            ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        });

        if let RenderingStrategy::Distributed(host) = &mut self.rendering_strategy {
            host.resize(width, height);
        }
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

        // if let Some(duck_entity) = self.duck_entity {
        //     let mut transforms_mut = self.world.entities_mut::<TransformComponent>();

        //     let duck_transform = transforms_mut.get_mut(duck_entity).unwrap();
        //     duck_transform.transform.translate(RIGHT * delta_time * 0.5);
        // }

        // {
        //     let mut transforms_mut = self.world.entities_mut::<TransformComponent>();
        //     let transform = transforms_mut.get_mut(self.toy_car_entity).unwrap();
        //     transform
        //         .transform
        //         .rotate(Quat::from_axis_angle(UP, delta_time * 0.3));
        // }

        // if self.input_handler.key(KeyCode::KeyX) {
        //     if let Some(duck_entity) = self.duck_entity.take() {
        //         self.world.destroy_entity(duck_entity);
        //     }
        // }

        if self.input_handler.key(KeyCode::Escape) {
            return true;
        }

        self.world.camera_mut(|camera| {
            camera.transform =
                self.camera_controller
                    .update(camera, &self.input_handler, delta_time);
        });

        match &mut self.rendering_strategy {
            RenderingStrategy::Distributed(host) => {
                if host.handle_new_connections() {
                    self.world.resync_all_visible_world_actions();
                } else {
                    self.world.finalize_visible_world_actions();
                }

                host.send_visible_world_actions(self.world.get_visible_world_actions());

                host.render(|pixels| {
                    ctx.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &self.texture[0],
                            mip_level: 0,
                            origin: Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        pixels,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * self.texture[0].width()),
                            rows_per_image: None,
                        },
                        Extent3d {
                            width: self.texture[0].width(),
                            height: self.texture[0].height(),
                            depth_or_array_layers: 1,
                        },
                    );
                });
            }
            RenderingStrategy::Local(distributed_renderer) => {
                self.world.finalize_visible_world_actions();
                let visible_world_actions = self.world.get_visible_world_actions();
                for action in visible_world_actions {
                    let visible_world_action =
                        VisibleWorldActionType::from_ty_and_bytes(action.ty, action.data.as_ref());

                    distributed_renderer.visible_world_action(&visible_world_action);
                }

                distributed_renderer.render(
                    UVec2::new(self.texture[0].width(), self.texture[0].height()),
                    0,
                    self.texture[0].height(),
                    |pixels| {
                        ctx.queue.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &self.texture[0],
                                mip_level: 0,
                                origin: Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            pixels,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4 * self.texture[0].width()),
                                rows_per_image: None,
                            },
                            Extent3d {
                                width: self.texture[0].width(),
                                height: self.texture[0].height(),
                                depth_or_array_layers: 1,
                            },
                        );
                    },
                );
            }
        }

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let unresolved_texture_view =
            self.texture[0].create_view(&wgpu::TextureViewDescriptor::default());
        let resolved_texture_view =
            self.texture[1].create_view(&wgpu::TextureViewDescriptor::default());

        block_to_linear_pass::encode(
            &BlockToLinearPassParameters {
                resolution: UVec2::new(self.texture[0].width(), self.texture[0].height()),
                target_view: &unresolved_texture_view,
                resolve_target_view: &resolved_texture_view,
            },
            &ctx.device,
            &mut command_encoder,
            &mut self.pipeline_database,
        );

        blit_pass::encode(
            &resolved_texture_view,
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
        resizeable: true,
        maximized: false,
        no_gpu_validation: Args::parse().no_gpu_validation,
    })
    .run()?;

    Ok(())
}
