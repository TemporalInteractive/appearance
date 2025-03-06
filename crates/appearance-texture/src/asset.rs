use anyhow::anyhow;
use anyhow::Result;
use appearance_asset_database::Asset;

use crate::Texture;
use crate::TextureCreateDesc;
use crate::TextureFormat;

impl Asset for Texture {
    fn load(file_path: &str, data: &[u8]) -> Result<Self> {
        let mut image = image::load_from_memory(data)?;

        if let image::DynamicImage::ImageRgb32F(_) = &image {
            image = image::DynamicImage::ImageRgba32F(image.to_rgba32f());
        }

        match image {
            image::DynamicImage::ImageRgb8(image) => Ok(Texture::new(TextureCreateDesc {
                name: Some(file_path.to_owned()),
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgb8Unorm,
                data: image.into_raw().into_boxed_slice(),
            })),
            image::DynamicImage::ImageRgba8(image) => Ok(Texture::new(TextureCreateDesc {
                name: Some(file_path.to_owned()),
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba8Unorm,
                data: image.into_raw().into_boxed_slice(),
            })),
            image::DynamicImage::ImageRgba32F(image) => Ok(Texture::new(TextureCreateDesc {
                name: Some(file_path.to_owned()),
                width: image.width(),
                height: image.height(),
                format: TextureFormat::Rgba32Float,
                data: bytemuck::cast_slice(&image.into_raw())
                    .to_vec()
                    .into_boxed_slice(),
            })),
            _ => Err(anyhow!("Format not supported yet!")),
        }
    }

    fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}
