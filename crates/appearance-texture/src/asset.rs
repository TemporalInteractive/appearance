use anyhow::anyhow;
use anyhow::Result;
use appearance_asset_database::Asset;

use crate::Texture;
use crate::TextureCreateDesc;
use crate::TextureFormat;

impl Asset for Texture {
    fn load(file_path: &str, data: &[u8]) -> Result<Self> {
        let image = image::load_from_memory(data)?;

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
            _ => Err(anyhow!("Format not supported yet!")),
        }
    }

    fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}
