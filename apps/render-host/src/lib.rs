use std::sync::Arc;

use appearance::appearance_render_loop::host::Host;
use appearance::appearance_render_loop::winit::window::Window;
use appearance::appearance_render_loop::{winit, RenderLoop, RenderLoopHandler};
use appearance::appearance_wgpu::helper_passes::blit_pass;
use appearance::appearance_wgpu::wgpu::{self, Extent3d, Origin3d};
use appearance::Appearance;

// use anyhow::Error;
// use std::fs::File;
// use std::io::prelude::*;
// use std::net::{TcpListener, TcpStream};
// use std::thread;

// fn handle_client(mut stream: TcpStream) -> Result<(), Error> {
//     let buf: &mut [u8; 100] = &mut [0; 100];
//     let mut file = File::create("foo")?;
//     let len = stream.read(buf)?;
//     let str = String::from_utf8(buf[0..len].to_vec());
//     file.write_all(&buf[0..len])?;

//     println!("wrote: {:?}", str);
//     Ok(())
// }

pub struct HostRenderLoop {
    host: Host,
    texture: wgpu::Texture,
    swapchain_format: wgpu::TextureFormat,
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
        let host = Host::new("127.0.0.1:34234".to_owned(), config.width, config.height).unwrap();

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

        Self {
            host,
            texture,
            swapchain_format: config.view_formats[0],
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        todo!()
    }

    fn window_event(&mut self, _event: winit::event::WindowEvent) {}

    fn render(&mut self, view: &wgpu::TextureView, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.host
            .render(|pixels| {
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
            })
            .unwrap();

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
    }
}

pub fn internal_main() {
    let _ = Appearance::new("Render Host");
    RenderLoopHandler::<HostRenderLoop>::new().run().unwrap();

    // let listener = TcpListener::bind("127.0.0.1:34234").unwrap();

    // for stream in listener.incoming() {
    //     match stream {
    //         Ok(stream) => {
    //             thread::spawn(move || handle_client(stream));
    //         }
    //         Err(_) => {
    //             break;
    //         }
    //     }
    // }
}
