use glam::{UVec2, Vec2, Vec4, Vec4Swizzles};
use half::f16;
use uuid::Uuid;

pub mod asset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba32Float,
    Rgba8Unorm,
    Rgb8Unorm,
    Rg8Unorm,
    R8Unorm,
}

impl TextureFormat {
    // BC6hRgbUfloat
    pub fn num_channels(&self) -> usize {
        match self {
            Self::R8Unorm => 1,
            Self::Rg8Unorm => 2,
            Self::Rgb8Unorm => 3,
            Self::Rgba8Unorm => 4,
            Self::Rgba32Float => 4,
        }
    }

    pub fn bytes_per_channel(&self) -> usize {
        match self {
            Self::R8Unorm | Self::Rg8Unorm | Self::Rgb8Unorm | Self::Rgba8Unorm => size_of::<u8>(),
            Self::Rgba32Float => size_of::<f16>(),
        }
    }

    pub fn to_wgpu(&self) -> wgpu::TextureFormat {
        match self {
            Self::R8Unorm => wgpu::TextureFormat::R8Unorm,
            Self::Rg8Unorm => wgpu::TextureFormat::Rg8Unorm,
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            _ => panic!("Failed to convert {:?} to wgpu.", self),
        }
    }

    pub fn to_wgpu_compressed(&self) -> wgpu::TextureFormat {
        match self {
            Self::R8Unorm => wgpu::TextureFormat::Bc4RUnorm,
            Self::Rg8Unorm => wgpu::TextureFormat::Bc5RgUnorm,
            Self::Rgba8Unorm => wgpu::TextureFormat::Bc7RgbaUnorm,
            Self::Rgba32Float => wgpu::TextureFormat::Bc6hRgbUfloat,
            _ => panic!("Failed to convert {:?} to wgpu.", self),
        }
    }
}

pub struct TextureCreateDesc {
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Box<[u8]>,
}

#[derive(Debug)]
pub struct Texture {
    name: String,
    width: u32,
    height: u32,
    format: TextureFormat,
    data: Box<[u8]>,
    uuid: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextureSampleRepeat {
    #[default]
    Clamp,
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextureSampleInterpolation {
    #[default]
    Nearest,
    Linear,
}

impl Texture {
    pub fn new(create_desc: TextureCreateDesc) -> Self {
        Self {
            name: create_desc.name.unwrap_or("Unnamed".to_owned()),
            width: create_desc.width,
            height: create_desc.height,
            format: create_desc.format,
            data: create_desc.data,
            uuid: Uuid::new_v4(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> TextureFormat {
        self.format
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn stride(&self) -> usize {
        self.format.num_channels()
    }

    pub fn load(&self, id: UVec2) -> Vec4 {
        let pixel_id = (id.y * self.width + id.x) as usize;

        let mut result = Vec4::ONE;
        for i in 0..self.format.num_channels() {
            result[i] = self.data[pixel_id * self.format.num_channels() + i] as f32 / 255.0;
        }
        result
    }

    pub fn sample(
        &self,
        uv: Vec2,
        repeat: TextureSampleRepeat,
        interpolation: TextureSampleInterpolation,
    ) -> Vec4 {
        match interpolation {
            TextureSampleInterpolation::Linear => {
                let x = (uv.x * self.width as f32).abs();
                let y = (uv.y * self.height as f32).abs();

                let tx = x.fract();
                let ty = y.fract();

                let id00 = UVec2::new(x as u32, y as u32);
                let id10 = UVec2::new(x as u32 + 1, y as u32);
                let id01 = UVec2::new(x as u32, y as u32 + 1);
                let id11 = UVec2::new(x as u32 + 1, y as u32 + 1);

                let (id00, id10, id01, id11) = match repeat {
                    TextureSampleRepeat::Clamp => (
                        id00.clamp(UVec2::ZERO, UVec2::new(self.width - 1, self.height - 1)),
                        id10.clamp(UVec2::ZERO, UVec2::new(self.width - 1, self.height - 1)),
                        id01.clamp(UVec2::ZERO, UVec2::new(self.width - 1, self.height - 1)),
                        id11.clamp(UVec2::ZERO, UVec2::new(self.width - 1, self.height - 1)),
                    ),
                    TextureSampleRepeat::Repeat => (
                        id00 % UVec2::new(self.width, self.height),
                        id10 % UVec2::new(self.width, self.height),
                        id01 % UVec2::new(self.width, self.height),
                        id11 % UVec2::new(self.width, self.height),
                    ),
                };

                let c00 = self.load(id00);
                let c10 = self.load(id10);
                let c01 = self.load(id01);
                let c11 = self.load(id11);

                bilinear(tx, ty, c00, c10, c01, c11)
            }
            TextureSampleInterpolation::Nearest => {
                let x = (uv.x * self.width as f32).abs();
                let y = (uv.y * self.height as f32).abs();

                let id = match repeat {
                    TextureSampleRepeat::Clamp => UVec2::new(
                        (x as u32).clamp(0, self.width - 1),
                        (y as u32).clamp(0, self.height - 1),
                    ),
                    TextureSampleRepeat::Repeat => {
                        UVec2::new((x as u32) % self.width, (y as u32) % self.height)
                    }
                };

                self.load(id)
            }
        }
    }

    pub fn get_sampling_distribution(&self) -> Vec<Vec<f32>> {
        let mut distribution = vec![vec![0.0; self.height as usize]; self.width as usize];

        // TODO: rayon par iter
        for y in 0..self.height {
            for x in 0..self.width {
                let value = self.load(UVec2::new(x, y)).xyz().element_sum() / 3.0;
                distribution[x as usize][y as usize] = value;
            }
        }

        distribution
    }

    pub fn create_wgpu_texture(
        &self,
        compressed: bool,
        usage: wgpu::TextureUsages,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let format = if compressed {
            self.format.to_wgpu_compressed()
        } else {
            self.format.to_wgpu()
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: usage | wgpu::TextureUsages::COPY_DST,
            label: Some(&self.name),
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        let (data, bytes_per_row) = if compressed {
            let data = match format {
                wgpu::TextureFormat::Bc4RUnorm => {
                    let surface = intel_tex_2::RSurface {
                        width: self.width,
                        height: self.height,
                        stride: self.width
                            * (self.format.num_channels() * self.format.bytes_per_channel()) as u32,
                        data: &self.data,
                    };

                    intel_tex_2::bc4::compress_blocks(&surface)
                }
                wgpu::TextureFormat::Bc5RgUnorm => {
                    let surface = intel_tex_2::RgSurface {
                        width: self.width,
                        height: self.height,
                        stride: self.width
                            * (self.format.num_channels() * self.format.bytes_per_channel()) as u32,
                        data: &self.data,
                    };

                    intel_tex_2::bc5::compress_blocks(&surface)
                }
                wgpu::TextureFormat::Bc6hRgbUfloat => {
                    let f32_data = bytemuck::cast_slice(&self.data);
                    let f16_data: Vec<f16> = f32_data.iter().copied().map(f16::from_f32).collect();

                    let surface = intel_tex_2::RgbaSurface {
                        width: self.width,
                        height: self.height,
                        stride: self.width
                            * (self.format.num_channels() * self.format.bytes_per_channel()) as u32,
                        data: bytemuck::cast_slice(&f16_data),
                    };

                    intel_tex_2::bc6h::compress_blocks(
                        &intel_tex_2::bc6h::very_fast_settings(),
                        &surface,
                    )
                }
                wgpu::TextureFormat::Bc7RgbaUnorm => {
                    let surface = intel_tex_2::RgbaSurface {
                        width: self.width,
                        height: self.height,
                        stride: self.width
                            * (self.format.num_channels() * self.format.bytes_per_channel()) as u32,
                        data: &self.data,
                    };

                    intel_tex_2::bc7::compress_blocks(
                        &intel_tex_2::bc7::alpha_ultra_fast_settings(),
                        &surface,
                    )
                }
                _ => panic!("Unsupported texture format."),
            };

            let block_size = match format {
                wgpu::TextureFormat::Bc1RgbaUnorm
                | wgpu::TextureFormat::Bc4RUnorm
                | wgpu::TextureFormat::Bc4RSnorm => 8,
                wgpu::TextureFormat::Bc2RgbaUnorm
                | wgpu::TextureFormat::Bc3RgbaUnorm
                | wgpu::TextureFormat::Bc5RgUnorm
                | wgpu::TextureFormat::Bc5RgSnorm
                | wgpu::TextureFormat::Bc6hRgbFloat
                | wgpu::TextureFormat::Bc6hRgbUfloat
                | wgpu::TextureFormat::Bc7RgbaUnorm => 16,
                _ => panic!("Unsupported texture format."),
            };
            let bytes_per_row = ((self.width + 3) / 4) * block_size;

            (data, bytes_per_row)
        } else {
            let bytes_per_row =
                self.width * (self.format.num_channels() * self.format.bytes_per_channel()) as u32;
            (self.data.to_vec(), bytes_per_row)
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        (texture, texture_view)
    }
}

fn bilinear(tx: f32, ty: f32, c00: Vec4, c10: Vec4, c01: Vec4, c11: Vec4) -> Vec4 {
    let a = c00 * (1.0 - tx) + c10 * tx;
    let b = c01 * (1.0 - tx) + c11 * tx;
    a * (1.0 - ty) + b * ty
}
