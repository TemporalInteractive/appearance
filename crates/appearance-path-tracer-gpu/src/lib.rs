#![allow(clippy::needless_range_loop)]

use appearance_camera::Camera;
use appearance_packing::PackedRgb9e5;
use appearance_wgpu::{pipeline_database::PipelineDatabase, wgpu, Context};
use appearance_world::visible_world_action::VisibleWorldActionType;
use apply_di_pass::ApplyDiPassParameters;
use demodulate_radiance::DemodulateRadiancePassParameters;
use film::Film;
use gbuffer::GBuffer;
use glam::{UVec2, Vec3};
use raygen_pass::RaygenPassParameters;
use resolve_pass::ResolvePassParameters;
use restir_di_pass::{LightSampleCtx, PackedDiReservoir, RestirDiPass, RestirDiPassParameters};
use scene_resources::SceneResources;
use taa_pass::TaaPassParameters;
use trace_pass::TracePassParameters;

mod apply_di_pass;
mod demodulate_radiance;
mod film;
mod gbuffer;
mod raygen_pass;
mod resolve_pass;
mod restir_di_pass;
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
    rays: [wgpu::Buffer; 2],
    payloads: wgpu::Buffer,
    radiance: wgpu::Buffer,
    demodulated_radiance: [wgpu::Buffer; 2],
    light_sample_reservoirs: wgpu::Buffer,
    light_sample_ctxs: wgpu::Buffer,
    gbuffer: GBuffer,

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

        let gbuffer = GBuffer::new(resolution, device);

        let restir_di_pass = RestirDiPass::new(resolution, device);

        Self {
            film,
            rays,
            payloads,
            radiance,
            demodulated_radiance,
            light_sample_reservoirs,
            light_sample_ctxs,
            gbuffer,
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
            max_bounces: 2,
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
                        radiance: &self.sized_resources.radiance,
                        light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                        light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                        gbuffer: &self.sized_resources.gbuffer,
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
                            seed: self.frame_idx,
                            spatial_pass_count: 2,
                            spatial_pixel_radius: 30.0,
                            in_rays,
                            payloads: &self.sized_resources.payloads,
                            light_sample_reservoirs: &self.sized_resources.light_sample_reservoirs,
                            light_sample_ctxs: &self.sized_resources.light_sample_ctxs,
                            gbuffer: &self.sized_resources.gbuffer,
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
                        radiance: &self.sized_resources.radiance,
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

        if self.frame_idx > 0 && false {
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

            taa_pass::encode(
                &TaaPassParameters {
                    resolution: self.local_resolution,
                    history_influence: 0.9,
                    demodulated_radiance,
                    prev_demodulated_radiance,
                    gbuffer: &self.sized_resources.gbuffer,
                },
                &ctx.device,
                &mut command_encoder,
                pipeline_database,
            );

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
        }

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
        self.sized_resources.gbuffer.end_frame(&self.camera);
        self.camera.end_frame();
    }
}
