use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    Rgba8Unorm,
    Rgb8Unorm,
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

    pub fn sample(&self, uv: Vec2) -> Vec3 {
        let x = uv.x * self.width as f32;
        let y = uv.y * self.height as f32;

        let x = ((x.abs() as i32) % self.width as i32) as u32;
        let y = ((y.abs() as i32) % self.height as i32) as u32;

        let pixel_id = (y * self.width + x) as usize;
        let r = self.data[pixel_id * 3] as f32 / 255.0;
        let g = self.data[pixel_id * 3 + 1] as f32 / 255.0;
        let b = self.data[pixel_id * 3 + 2] as f32 / 255.0;

        Vec3::new(r, g, b)
    }
}
