use appearance_camera::Camera;
use appearance_wgpu::{pipeline_database::PipelineDatabase, wgpu, Context};
use appearance_world::visible_world_action::VisibleWorldActionType;
use film::Film;
use glam::{UVec2, Vec3};
use raygen_pass::RaygenPassParameters;
use resolve_pass::ResolvePassParameters;
use trace_pass::TracePassParameters;

mod film;
mod raygen_pass;
mod resolve_pass;
mod trace_pass;

#[repr(C)]
struct Ray {
    origin: Vec3,
    _padding0: u32,
    direction: Vec3,
    _padding1: u32,
}

#[repr(C)]
struct Payload {
    accumulated: Vec3,
    _padding0: u32,
}

struct SizedResources {
    film: Film,
    rays: wgpu::Buffer,
    payloads: wgpu::Buffer,
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

        Self {
            film,
            rays,
            payloads,
        }
    }
}

pub struct PathTracerGpu {
    resolution: UVec2,
    local_resolution: UVec2,
    sized_resources: SizedResources,
    camera: Camera,
}

impl PathTracerGpu {
    pub fn new(ctx: &Context) -> Self {
        let resolution = UVec2::new(1920, 1080);
        let sized_resources = SizedResources::new(resolution, &ctx.device);

        Self {
            resolution,
            local_resolution: resolution,
            sized_resources,
            camera: Camera::default(),
        }
    }

    pub fn handle_visible_world_action(&mut self, action: &VisibleWorldActionType) {
        if let VisibleWorldActionType::CameraUpdate(data) = action {
            self.camera.set_near(data.near);
            self.camera.set_far(data.far);
            self.camera.set_fov(data.fov);
            self.camera
                .transform
                .set_matrix(data.transform_matrix_bytes);
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
        self.resize(resolution, start_row, end_row, ctx);

        self.camera
            .set_aspect_ratio(resolution.x as f32 / resolution.y as f32);
        let inv_view = self.camera.transform.get_matrix();
        let inv_proj = self.camera.get_matrix().inverse();

        let mut command_encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

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

        trace_pass::encode(
            &TracePassParameters {
                ray_count: self.local_resolution.x * self.local_resolution.y,
                rays: &self.sized_resources.rays,
                payloads: &self.sized_resources.payloads,
            },
            &ctx.device,
            &mut command_encoder,
            pipeline_database,
        );

        resolve_pass::encode(
            &ResolvePassParameters {
                resolution: self.local_resolution,
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
    }
}
