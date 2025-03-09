#![allow(clippy::needless_range_loop)]

use appearance_camera::Camera;
use appearance_wgpu::{pipeline_database::PipelineDatabase, wgpu, Context};
use appearance_world::visible_world_action::VisibleWorldActionType;
use apply_di_pass::ApplyDiPassParameters;
use film::Film;
use glam::{UVec2, Vec3};
use raygen_pass::RaygenPassParameters;
use resolve_pass::ResolvePassParameters;
use restir_di_pass::{LightSampleCtx, PackedDiReservoir, RestirDiPass, RestirDiPassParameters};
use scene_resources::SceneResources;
use trace_pass::TracePassParameters;

mod apply_di_pass;
mod film;
mod raygen_pass;
mod resolve_pass;
mod restir_di_pass;
mod scene_resources;
mod trace_pass;

#[repr(C)]
struct Ray {
    origin: Vec3,
    direction: u32,
}

#[repr(C)]
struct Payload {
    accumulated: u32,
    throughput: u32,
    rng: u32,
    t: f32,
}

struct SizedResources {
    film: Film,
    rays: [wgpu::Buffer; 2],
    payloads: wgpu::Buffer,
    light_sample_reservoirs: wgpu::Buffer,
    light_sample_ctxs: wgpu::Buffer,

    restir_di_pass: RestirDiPass,
}

impl SizedResources {
    fn new(resolution: UVec2, device: &wgpu::Device) -> Self {
        let film = Film::new(resolution, device);

        let rays = std::array::from_fn(|i| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("appearance-path-tracer-gpu rays {}", i)),
                size: (std::mem::size_of::<Ray>() as u32 * resolution.x * resolution.y) as u64,
                mapped_at_creation: false,
                usage: wgpu::BufferUsages::STORAGE,
            })
        });

        let payloads = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu payloads"),
            size: (std::mem::size_of::<Payload>() as u32 * resolution.x * resolution.y) as u64,
            mapped_at_creation: false,
            usage: wgpu::BufferUsages::STORAGE,
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

        let restir_di_pass = RestirDiPass::new(resolution, device);

        Self {
            film,
            rays,
            payloads,
            light_sample_reservoirs,
            light_sample_ctxs,
            restir_di_pass,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PathTracerGpuConfig {
    max_bounces: u32,
    sample_count: u32,
}

impl Default for PathTracerGpuConfig {
    fn default() -> Self {
        Self {
            max_bounces: 5,
            sample_count: 1,
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
                self.camera.set_near(data.near);
                self.camera.set_far(data.far);
                self.camera.set_fov(data.fov);
                self.camera
                    .transform
                    .set_matrix(data.transform_matrix_bytes);
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
        let inv_view = self.camera.transform.get_matrix();
        let inv_proj = self.camera.get_matrix().inverse();

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        self.scene_resources
            .rebuild_tlas(&mut command_encoder, &ctx.queue);

        for sample in 0..self.config.sample_count {
            raygen_pass::encode(
                &RaygenPassParameters {
                    inv_view,
                    inv_proj,
                    resolution: self.local_resolution,
                    rays: &self.sized_resources.rays[0],
                },
                &ctx.device,
                &mut command_encoder,
                pipeline_database,
            );

            for i in 0..self.config.max_bounces {
                let in_rays = &self.sized_resources.rays[(i as usize) % 2];
                let out_rays = &self.sized_resources.rays[(i as usize + 1) % 2];

                let seed = self.frame_idx * self.config.sample_count + sample;

                trace_pass::encode(
                    &TracePassParameters {
                        ray_count: self.local_resolution.x * self.local_resolution.y,
                        bounce: i,
                        seed,
                        sample,
                        in_rays,
                        out_rays,
                        payloads: &self.sized_resources.payloads,
                        light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                        light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                        scene_resources: &self.scene_resources,
                    },
                    &ctx.device,
                    &mut command_encoder,
                    pipeline_database,
                );

                if i == 0 {
                    self.sized_resources.restir_di_pass.encode(
                        &RestirDiPassParameters {
                            resolution: self.local_resolution,
                            spatial_pass_count: 2,
                            in_rays,
                            payloads: &self.sized_resources.payloads,
                            light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                            light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                            scene_resources: &self.scene_resources,
                        },
                        &ctx.device,
                        &mut command_encoder,
                        pipeline_database,
                    );
                }

                apply_di_pass::encode(
                    &ApplyDiPassParameters {
                        ray_count: self.local_resolution.x * self.local_resolution.y,
                        in_rays,
                        payloads: &self.sized_resources.payloads,
                        light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                        light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                        scene_resources: &self.scene_resources,
                    },
                    &ctx.device,
                    &mut command_encoder,
                    pipeline_database,
                );
            }
        }

        resolve_pass::encode(
            &ResolvePassParameters {
                resolution: self.local_resolution,
                sample_count: self.config.sample_count,
                payloads: &self.sized_resources.payloads,
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
    }
}
