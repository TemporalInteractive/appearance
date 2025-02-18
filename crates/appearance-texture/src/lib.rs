use glam::{UVec2, Vec2, Vec4};

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

        let mut result = Vec4::ZERO;
        for i in 0..self.format.num_channels() {
            result[i] = self.data[pixel_id * self.format.num_channels() + i] as f32 / 255.0;
        }
        result
    }

    pub fn sample(&self, uv: Vec2) -> Vec4 {
        let x = (uv.x * self.width as f32).abs();
        let y = (uv.y * self.height as f32).abs();

        let tx = x.fract();
        let ty = y.fract();

        let id00 = UVec2::new((x as u32) % self.width, (y as u32) % self.height);
        let id10 = UVec2::new((x as u32 + 1) % self.width, (y as u32) % self.height);
        let id01 = UVec2::new((x as u32) % self.width, (y as u32 + 1) % self.height);
        let id11 = UVec2::new((x as u32 + 1) % self.width, (y as u32 + 1) % self.height);

        let c00 = self.load(id00);
        let c10 = self.load(id10);
        let c01 = self.load(id01);
        let c11 = self.load(id11);

        bilinear(tx, ty, c00, c10, c01, c11)
    }
}

fn bilinear(tx: f32, ty: f32, c00: Vec4, c10: Vec4, c01: Vec4, c11: Vec4) -> Vec4 {
    let a = c00 * (1.0 - tx) + c10 * tx;
    let b = c01 * (1.0 - tx) + c11 * tx;
    a * (1.0 - ty) + b * ty
}
