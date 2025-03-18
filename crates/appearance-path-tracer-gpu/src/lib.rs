#![allow(clippy::needless_range_loop)]

use appearance_camera::Camera;
use appearance_packing::PackedRgb9e5;
use appearance_wgpu::{pipeline_database::PipelineDatabase, wgpu, Context};
use appearance_world::visible_world_action::VisibleWorldActionType;
use apply_di_pass::ApplyDiPassParameters;
use apply_gi_pass::ApplyGiPassParameters;
use demodulate_radiance::DemodulateRadiancePassParameters;
use film::Film;
use firefly_filter_pass::FireflyFilterPassParameters;
use gbuffer::GBuffer;
use gbuffer_pass::GbufferPassParameters;
use glam::{UVec2, Vec3};
use raygen_pass::RaygenPassParameters;
use resolve_pass::ResolvePassParameters;
use restir_di_pass::{LightSampleCtx, PackedDiReservoir, RestirDiPass, RestirDiPassParameters};
use restir_gi_pass::{PackedGiReservoir, RestirGiPass, RestirGiPassParameters};
use scene_resources::SceneResources;
use taa_pass::TaaPassParameters;
use trace_pass::TracePassParameters;

mod apply_di_pass;
mod apply_gi_pass;
mod demodulate_radiance;
mod film;
mod firefly_filter_pass;
mod gbuffer;
mod gbuffer_pass;
mod raygen_pass;
mod resolve_pass;
mod restir_di_pass;
mod restir_gi_pass;
mod scene_resources;
mod taa_pass;
mod trace_pass;

#[repr(C)]
struct Ray {
    origin: Vec3,
    direction: u32,
}

#[repr(C)]
struct Payload {
    throughput: u32,
    rng: u32,
    t: f32,
    _padding0: u32,
}

struct SizedResources {
    film: Film,
    rays: wgpu::Buffer,
    payloads: wgpu::Buffer,
    radiance: wgpu::Buffer,
    demodulated_radiance: [wgpu::Buffer; 2],
    light_sample_reservoirs: wgpu::Buffer,
    light_sample_ctxs: wgpu::Buffer,
    gi_reservoirs: wgpu::Buffer,
    gbuffer: GBuffer,
    velocity_texture_view: wgpu::TextureView,
    depth_texture: wgpu::Texture,

    restir_di_pass: RestirDiPass,
    restir_gi_pass: RestirGiPass,
}

impl SizedResources {
    fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let film = Film::new(resolution, device);

        let rays = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu rays"),
            size: (std::mem::size_of::<Ray>() as u32 * resolution.x * resolution.y) as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let payloads = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu payloads"),
            size: (std::mem::size_of::<Payload>() as u32 * resolution.x * resolution.y) as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let radiance = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu radiance"),
            size: (std::mem::size_of::<PackedRgb9e5>() as u32 * resolution.x * resolution.y) as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let demodulated_radiance = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!(
                    "appearance-path-tracer-gpu demodulated_radiance {}",
                    i
                )),
                size: (std::mem::size_of::<PackedRgb9e5>() as u32 * resolution.x * resolution.y)
                    as u64,
                mapped_at_creation: false,
                usage: wgpu::BufferUsages::STORAGE,
            })
        });

        let light_sample_reservoirs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu light_sample_reservoirs"),
            size: (std::mem::size_of::<PackedDiReservoir>() as u32 * resolution.x * resolution.y)
                as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let light_sample_ctxs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu light_sample_ctxs"),
            size: (std::mem::size_of::<LightSampleCtx>() as u32 * resolution.x * resolution.y)
                as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let gi_reservoirs = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu gi_reservoirs"),
            size: (std::mem::size_of::<PackedGiReservoir>() as u32 * resolution.x * resolution.y)
                as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let gbuffer = GBuffer::new(resolution, device);

        let velocity_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("appearance-path-tracer-gpu velocity"),
            size: wgpu::Extent3d {
                width: resolution.x,
                height: resolution.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let velocity_texture_view =
            velocity_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("appearance-path-tracer-gpu depth"),
            size: wgpu::Extent3d {
                width: resolution.x,
                height: resolution.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let restir_di_pass = RestirDiPass::new(resolution, device);
        let restir_gi_pass = RestirGiPass::new(resolution, device);

        Self {
            film,
            rays,
            payloads,
            radiance,
            demodulated_radiance,
            light_sample_reservoirs,
            light_sample_ctxs,
            gi_reservoirs,
            gbuffer,
            velocity_texture_view,
            depth_texture,
            restir_di_pass,
            restir_gi_pass,
        }
    }

    fn end_frame(&mut self, camera: &Camera) {
        self.gbuffer.end_frame(camera);
        self.restir_di_pass.end_frame();
        self.restir_gi_pass.end_frame();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PathTracerGpuConfig {
    max_bounces: u32,
    sample_count: u32,

    restir_di: bool,
    restir_gi: bool,
    firefly_filter: bool,
    taa: bool,
}

impl Default for PathTracerGpuConfig {
    fn default() -> Self {
        Self {
            max_bounces: 2,
            sample_count: 8,
            restir_di: true,
            restir_gi: false,
            firefly_filter: false,
            taa: false,
        }
    }
}

pub struct PathTracerGpu {
    config: PathTracerGpuConfig,
    resolution: UVec2,
    local_resolution: UVec2,
    sized_resources: SizedResources,
    camera: Camera,
    scene_resources: SceneResources,
    frame_idx: u32,

    upload_command_encoder: Option<wgpu::CommandEncoder>,
}

impl PathTracerGpu {
    pub fn new(ctx: &Context, config: PathTracerGpuConfig) -> Self {
        let resolution = UVec2::new(1920, 1080);
        let sized_resources = SizedResources::new(resolution, &ctx.device);

        let scene_resources = SceneResources::new(&ctx.device, &ctx.queue);

        let upload_command_encoder = Some(
            ctx.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None }),
        );

        Self {
            config,
            resolution,
            local_resolution: resolution,
            sized_resources,
            camera: Camera::default(),
            scene_resources,
            upload_command_encoder,
            frame_idx: 0,
        }
    }

    pub fn handle_visible_world_action(&mut self, action: &VisibleWorldActionType, ctx: &Context) {
        match action {
            VisibleWorldActionType::CameraUpdate(data) => {
                let (_, rotation, translation) =
                    data.transform_matrix_bytes.to_scale_rotation_translation();

                self.camera.set_near(data.near);
                self.camera.set_far(data.far);
                self.camera.set_fov(data.fov);
                self.camera.transform.set_rotation(rotation);
                self.camera.transform.set_translation(translation);
            }
            _ => self.scene_resources.handle_visible_world_action(
                action,
                self.upload_command_encoder.as_mut().unwrap(),
                &ctx.device,
                &ctx.queue,
            ),
        }
    }

    fn resize(&mut self, resolution: UVec2, start_row: u32, end_row: u32, ctx: &Context) {
        let local_resolution = UVec2::new(resolution.x, end_row - start_row);

        if self.resolution != resolution || self.local_resolution != local_resolution {
            self.resolution = resolution;
            self.local_resolution = local_resolution;
            self.sized_resources = SizedResources::new(self.local_resolution, &ctx.device);
        }
    }

    pub fn render<F: FnMut(&[u8])>(
        &mut self,
        resolution: UVec2,
        start_row: u32,
        end_row: u32,
        mut result_callback: F,
        ctx: &Context,
        pipeline_database: &mut PipelineDatabase,
    ) {
        if let Some(upload_command_encoder) = self.upload_command_encoder.take() {
            ctx.queue.submit(Some(upload_command_encoder.finish()));
            self.upload_command_encoder = Some(
                ctx.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None }),
            );
        }

        self.resize(resolution, start_row, end_row, ctx);

        self.camera
            .set_aspect_ratio(resolution.x as f32 / resolution.y as f32);
        let inv_view = self.camera.transform.get_view_matrix().inverse();
        let inv_proj = self.camera.get_matrix().inverse();
        let view_proj = self.camera.get_matrix() * self.camera.transform.get_view_matrix();
        let prev_view_proj =
            self.camera.get_prev_matrix() * self.camera.transform.get_prev_matrix();

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        self.scene_resources
            .rebuild_tlas(&mut command_encoder, &ctx.queue);

        let demodulated_radiance =
            &self.sized_resources.demodulated_radiance[(self.frame_idx as usize) % 2];
        let prev_demodulated_radiance =
            &self.sized_resources.demodulated_radiance[(self.frame_idx as usize + 1) % 2];

        command_encoder.clear_buffer(&self.sized_resources.radiance, 0, None);

        gbuffer_pass::encode(
            &GbufferPassParameters {
                view_proj,
                prev_view_proj,
                scene_resources: &self.scene_resources,
                gbuffer_view: &self.sized_resources.velocity_texture_view,
                depth_texture: &self.sized_resources.depth_texture,
            },
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        for sample in 0..self.config.sample_count {
            raygen_pass::encode(
                &RaygenPassParameters {
                    inv_view,
                    inv_proj,
                    resolution: self.local_resolution,
                    rays: &self.sized_resources.rays,
                },
                &ctx.device,
                &mut command_encoder,
                pipeline_database,
            );

            for i in 0..self.config.max_bounces {
                let seed = self.frame_idx * self.config.sample_count + sample;

                trace_pass::encode(
                    &TracePassParameters {
                        ray_count: self.local_resolution.x * self.local_resolution.y,
                        bounce: i,
                        max_bounces: self.config.max_bounces,
                        seed,
                        sample,
                        rays: &self.sized_resources.rays,
                        payloads: &self.sized_resources.payloads,
                        radiance: &self.sized_resources.radiance,
                        light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                        light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                        gi_reservoirs: &self.sized_resources.gi_reservoirs,
                        gbuffer: &self.sized_resources.gbuffer,
                        scene_resources: &self.scene_resources,
                    },
                    &ctx.device,
                    &mut command_encoder,
                    pipeline_database,
                );

                if i == 0 {
                    if self.config.restir_di {
                        self.sized_resources.restir_di_pass.encode(
                            &RestirDiPassParameters {
                                resolution: self.local_resolution,
                                seed: self.frame_idx,
                                spatial_pass_count: 1,
                                spatial_pixel_radius: 30.0,
                                unbiased: true,
                                rays: &self.sized_resources.rays,
                                payloads: &self.sized_resources.payloads,
                                light_sample_reservoirs: &self
                                    .sized_resources
                                    .light_sample_reservoirs,
                                light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                                gbuffer: &self.sized_resources.gbuffer,
                                velocity_texture_view: &self.sized_resources.velocity_texture_view,
                                scene_resources: &self.scene_resources,
                            },
                            &ctx.device,
                            &mut command_encoder,
                            pipeline_database,
                        );
                    }

                    if self.config.restir_gi {
                        self.sized_resources.restir_gi_pass.encode(
                            &RestirGiPassParameters {
                                resolution: self.local_resolution,
                                seed: self.frame_idx,
                                spatial_pass_count: 1,
                                spatial_pixel_radius: 30.0,
                                unbiased: true,
                                rays: &self.sized_resources.rays,
                                payloads: &self.sized_resources.payloads,
                                reservoirs: &self.sized_resources.gi_reservoirs,
                                light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                                gbuffer: &self.sized_resources.gbuffer,
                                velocity_texture_view: &self.sized_resources.velocity_texture_view,
                                scene_resources: &self.scene_resources,
                            },
                            &ctx.device,
                            &mut command_encoder,
                            pipeline_database,
                        );
                    }
                }

                apply_di_pass::encode(
                    &ApplyDiPassParameters {
                        ray_count: self.local_resolution.x * self.local_resolution.y,
                        rays: &self.sized_resources.rays,
                        payloads: &self.sized_resources.payloads,
                        radiance: &self.sized_resources.radiance,
                        light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                        light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                        scene_resources: &self.scene_resources,
                    },
                    &ctx.device,
                    &mut command_encoder,
                    pipeline_database,
                );

                if i + 1 < self.config.max_bounces {
                    apply_gi_pass::encode(
                        &ApplyGiPassParameters {
                            ray_count: self.local_resolution.x * self.local_resolution.y,
                            rays: &self.sized_resources.rays,
                            payloads: &self.sized_resources.payloads,
                            gi_reservoirs: &self.sized_resources.gi_reservoirs,
                            light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                            scene_resources: &self.scene_resources,
                        },
                        &ctx.device,
                        &mut command_encoder,
                        pipeline_database,
                    );
                }
            }
        }

        demodulate_radiance::encode(
            &DemodulateRadiancePassParameters {
                resolution: self.local_resolution,
                remodulate: false,
                in_radiance: &self.sized_resources.radiance,
                out_radiance: demodulated_radiance,
                gbuffer: &self.sized_resources.gbuffer,
            },
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        if self.config.firefly_filter {
            firefly_filter_pass::encode(
                &FireflyFilterPassParameters {
                    resolution: self.local_resolution,
                    demodulated_radiance,
                    gbuffer: &self.sized_resources.gbuffer,
                },
                &ctx.device,
                &mut command_encoder,
                pipeline_database,
            );
        }

        if self.config.taa && self.frame_idx > 0 {
            taa_pass::encode(
                &TaaPassParameters {
                    resolution: self.local_resolution,
                    history_influence: 0.8,
                    demodulated_radiance,
                    prev_demodulated_radiance,
                    gbuffer: &self.sized_resources.gbuffer,
                    velocity_texture_view: &self.sized_resources.velocity_texture_view,
                },
                &ctx.device,
                &mut command_encoder,
                pipeline_database,
            );
        }

        demodulate_radiance::encode(
            &DemodulateRadiancePassParameters {
                resolution: self.local_resolution,
                remodulate: true,
                in_radiance: demodulated_radiance,
                out_radiance: &self.sized_resources.radiance,
                gbuffer: &self.sized_resources.gbuffer,
            },
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        resolve_pass::encode(
            &ResolvePassParameters {
                resolution: self.local_resolution,
                sample_count: self.config.sample_count,
                radiance: &self.sized_resources.radiance,
                gbuffer: &self.sized_resources.gbuffer,
                target_view: self.sized_resources.film.texture_view(),
            },
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        self.sized_resources
            .film
            .prepare_pixel_readback(&mut command_encoder);

        ctx.queue.submit(Some(command_encoder.finish()));

        let pixels = self.sized_resources.film.readback_pixels(&ctx.device);
        result_callback(&pixels);

        self.frame_idx += 1;
        self.scene_resources.end_frame();
        self.sized_resources.end_frame(&self.camera);
        self.camera.end_frame();
    }
}
