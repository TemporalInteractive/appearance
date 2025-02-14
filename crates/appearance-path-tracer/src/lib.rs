#![allow(dead_code)]

use appearance_camera::Camera;

mod camera_model;
mod geometry_resources;
mod light_sources;
mod path_integrator;
mod path_tracer;
mod radiometry;
mod reflectance;
mod sampling;
use camera_model::{film::Film, pixel_sensor::PixelSensor};
use glam::{UVec2, Vec2};
mod math;

use appearance_render_loop::host::RENDER_BLOCK_SIZE;
use appearance_world::visible_world_action::VisibleWorldActionType;
use geometry_resources::*;
use path_tracer::{CameraMatrices, PATH_TRACER_RAY_PACKET_SIZE, RAYS_PER_PACKET};
use radiometry::{DenselySampledSpectrum, PiecewiseLinearSpectrum, RgbColorSpace};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

pub struct PathTracer {
    film: Film,

    frame_idx: u32,
    camera: Camera,

    geometry_resources: GeometryResources,
}

impl Default for PathTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl PathTracer {
    pub fn new() -> Self {
        rayon::ThreadPoolBuilder::new()
            .stack_size(1024 * 1024 * 512)
            .build_global()
            .unwrap();

        let pixel_sensor = PixelSensor::new(
            PiecewiseLinearSpectrum::canon_eos_100d_r().clone(),
            PiecewiseLinearSpectrum::canon_eos_100d_g().clone(),
            PiecewiseLinearSpectrum::canon_eos_100d_b().clone(),
            &RgbColorSpace::aces(),
            &DenselySampledSpectrum::cie_d(6500.0),
            1.0,
        );
        let film = Film::new(UVec2::new(512, 512), pixel_sensor, RgbColorSpace::srgb());

        Self {
            film,
            frame_idx: 0,
            camera: Camera::default(),
            geometry_resources: GeometryResources::new(),
        }
    }

    pub fn handle_visible_world_action(&mut self, action: &VisibleWorldActionType) {
        match action {
            VisibleWorldActionType::CameraUpdate(data) => {
                self.camera.set_near(data.near);
                self.camera.set_far(data.far);
                self.camera.set_fov(data.fov);
                self.camera
                    .transform
                    .set_matrix(data.transform_matrix_bytes);
            }
            _ => self.geometry_resources.handle_visible_world_action(action),
        }
    }

    pub fn render<F: FnMut(&[u8])>(
        &mut self,
        width: u32,
        height: u32,
        start_row: u32,
        end_row: u32,
        mut result_callback: F,
    ) {
        let num_rows = end_row - start_row;
        let num_blocks_x = width / RENDER_BLOCK_SIZE;
        let num_blocks_y = num_rows / RENDER_BLOCK_SIZE;

        self.film.resize(UVec2::new(width, num_rows));

        self.camera.set_aspect_ratio(width as f32 / height as f32);

        let camera_matrices = CameraMatrices {
            inv_view: self.camera.transform.get_matrix(),
            inv_proj: self.camera.get_matrix().inverse(),
        };

        self.geometry_resources.rebuild_tlas();

        // Loop over the number of blocks, flattened to allow for better multithreading utilization
        let flat_block_indices = (0..(num_blocks_y * num_blocks_x)).collect::<Vec<u32>>();
        flat_block_indices
            .into_par_iter()
            .for_each(|flat_block_idx| {
                let local_block_y = flat_block_idx / num_blocks_x;
                let local_block_x = flat_block_idx % num_blocks_x;

                // Loop over the block size, divided by the number of rays per packet
                for block_y in 0..(RENDER_BLOCK_SIZE / PATH_TRACER_RAY_PACKET_SIZE) {
                    for block_x in 0..(RENDER_BLOCK_SIZE / PATH_TRACER_RAY_PACKET_SIZE) {
                        let mut ray_uvs = [Vec2::ZERO; RAYS_PER_PACKET];

                        // Loop over the ray packets
                        for ray_block_y in 0..PATH_TRACER_RAY_PACKET_SIZE {
                            for ray_block_x in 0..PATH_TRACER_RAY_PACKET_SIZE {
                                let block_y = block_y * PATH_TRACER_RAY_PACKET_SIZE + ray_block_y;
                                let block_x = block_x * PATH_TRACER_RAY_PACKET_SIZE + ray_block_x;

                                let local_x = (local_block_x * RENDER_BLOCK_SIZE) + block_x;
                                let local_y = (local_block_y * RENDER_BLOCK_SIZE) + block_y;

                                let x = local_x;
                                let y = local_y + start_row;

                                let uv = Vec2::new(
                                    (x as f32 + 0.5) / width as f32,
                                    (y as f32 + 0.5) / height as f32,
                                ) * 2.0
                                    - 1.0;

                                let i = (ray_block_y * PATH_TRACER_RAY_PACKET_SIZE + ray_block_x)
                                    as usize;
                                ray_uvs[i] = uv;
                            }
                        }

                        let result = path_tracer::render_pixels(
                            ray_uvs,
                            self.frame_idx as u64,
                            &camera_matrices,
                            &self.geometry_resources,
                            width,
                            height,
                        );

                        for ray_block_y in 0..PATH_TRACER_RAY_PACKET_SIZE {
                            for ray_block_x in 0..PATH_TRACER_RAY_PACKET_SIZE {
                                let block_y = block_y * PATH_TRACER_RAY_PACKET_SIZE + ray_block_y;
                                let block_x = block_x * PATH_TRACER_RAY_PACKET_SIZE + ray_block_x;

                                let block_size = RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE;
                                let start_pixel = (local_block_y * block_size * num_blocks_x)
                                    + local_block_x * block_size;

                                let local_block_id = block_y * RENDER_BLOCK_SIZE + block_x;
                                let local_id = (start_pixel + local_block_id) as usize;

                                let i = (ray_block_y * PATH_TRACER_RAY_PACKET_SIZE + ray_block_x)
                                    as usize;

                                // A lot of performance is safed by not using a mutex for pixel access from multiple threads
                                unsafe {
                                    self.film.add_sample(
                                        local_id,
                                        &result[i].sampled_spectrum,
                                        &result[i].sampled_wavelengths,
                                    );
                                }
                            }
                        }
                    }
                }
            });

        self.frame_idx += 1;

        result_callback(self.film.pixels());
    }
}
