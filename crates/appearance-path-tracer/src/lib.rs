#![allow(dead_code)]
#![allow(unused_imports)]

use std::sync::{Arc, Mutex};

use appearance_camera::Camera;

mod geometry_resources;
mod path_tracer;
mod radiometry;
use glam::{Vec2, Vec3};
mod math;

use appearance_render_loop::host::{NODE_BYTES_PER_PIXEL, RENDER_BLOCK_SIZE};
use appearance_world::visible_world_action::VisibleWorldActionType;
use geometry_resources::*;
use path_tracer::CameraMatrices;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

/// Enables access to pixel data from multiple threads without any safety checks.
struct PixelDataPtr(*mut u8);

impl PixelDataPtr {
    fn new(pixels: &mut Vec<u8>) -> Self {
        Self(pixels.as_mut_ptr())
    }

    unsafe fn write_pixel(&self, i: usize, color: Vec3) {
        *self.0.add(i * NODE_BYTES_PER_PIXEL) = (color.x * 255.0) as u8;
        *self.0.add(i * NODE_BYTES_PER_PIXEL + 1) = (color.y * 255.0) as u8;
        *self.0.add(i * NODE_BYTES_PER_PIXEL + 2) = (color.z * 255.0) as u8;
    }
}

unsafe impl Send for PixelDataPtr {}
unsafe impl Sync for PixelDataPtr {}

pub struct PathTracer {
    pixels: Vec<u8>,
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
        Self {
            pixels: Vec::new(),
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

        self.pixels
            .resize((width * num_rows) as usize * NODE_BYTES_PER_PIXEL, 128);

        self.camera.set_aspect_ratio(width as f32 / height as f32);

        let camera_matrices = CameraMatrices {
            inv_view: self.camera.transform.get_matrix(),
            inv_proj: self.camera.get_matrix().inverse(),
        };

        self.geometry_resources.rebuild_tlas();

        let pixel_data_ptr = PixelDataPtr::new(&mut self.pixels);

        let flat_block_indices = (0..(num_blocks_y * num_blocks_x)).collect::<Vec<u32>>();
        flat_block_indices
            .into_par_iter()
            .for_each(|flat_block_idx| {
                let local_block_y = flat_block_idx / num_blocks_x;
                let local_block_x = flat_block_idx % num_blocks_x;

                for block_y in 0..RENDER_BLOCK_SIZE {
                    for block_x in 0..RENDER_BLOCK_SIZE {
                        let local_x = (local_block_x * RENDER_BLOCK_SIZE) + block_x;
                        let local_y = (local_block_y * RENDER_BLOCK_SIZE) + block_y;

                        let x = local_x;
                        let y = local_y + start_row;

                        let uv = Vec2::new(
                            (x as f32 + 0.5) / width as f32,
                            (y as f32 + 0.5) / height as f32,
                        ) * 2.0
                            - 1.0;

                        let result = path_tracer::render_pixel(
                            &uv,
                            &camera_matrices,
                            &self.geometry_resources,
                        );

                        let block_size = RENDER_BLOCK_SIZE * RENDER_BLOCK_SIZE;
                        let start_pixel = (local_block_y * block_size * num_blocks_x)
                            + local_block_x * block_size;

                        let local_block_id = block_y * RENDER_BLOCK_SIZE + block_x;
                        let local_id = (start_pixel + local_block_id) as usize;

                        // A lot of performance is safed by not using a mutex for pixel access from multiple threads
                        unsafe {
                            pixel_data_ptr.write_pixel(local_id, result);
                        }
                    }
                }
            });

        self.frame_idx += 1;

        result_callback(self.pixels.as_ref());
    }
}
