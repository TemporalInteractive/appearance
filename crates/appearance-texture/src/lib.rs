use glam::{UVec2, Vec2, Vec4, Vec4Swizzles};

pub mod asset;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgb8Unorm,
}

impl TextureFormat {
    pub fn num_channels(&self) -> usize {
        match self {
            Self::Rgb8Unorm => 3,
            Self::Rgba8Unorm => 4,
        }
    }
}

pub struct TextureCreateDesc {
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub data: Box<[u8]>,
}

#[derive(Debug)]
pub struct Texture {
    width: u32,
    height: u32,
    format: TextureFormat,
    data: Box<[u8]>,
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
            width: create_desc.width,
            height: create_desc.height,
            format: create_desc.format,
            data: create_desc.data,
        }
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
}

fn bilinear(tx: f32, ty: f32, c00: Vec4, c10: Vec4, c01: Vec4, c11: Vec4) -> Vec4 {
    let a = c00 * (1.0 - tx) + c10 * tx;
    let b = c01 * (1.0 - tx) + c11 * tx;
    a * (1.0 - ty) + b * ty
}
