use std::{collections::HashMap, fs, io::Read, sync::Arc};

use anyhow::Result;

pub trait Asset
where
    Self: Sized,
{
    fn load(file_path: &str, data: &[u8]) -> Result<Self>;
}

pub struct AssetDatabase<A: Asset> {
    assets: HashMap<String, Arc<A>>,
}

impl<A: Asset> Default for AssetDatabase<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Asset> AssetDatabase<A> {
    pub fn new() -> Self {
        Self {
            assets: HashMap::new(),
        }
    }

    pub fn get(&mut self, path: &str) -> Result<Arc<A>> {
        appearance_profiling::profile_function!();

        if let Some(asset) = self.assets.get(path) {
            Ok(asset.clone())
        } else {
            let mut file = fs::File::open(path)?;
            let metadata = fs::metadata(path)?;
            let mut data = vec![0; metadata.len() as usize];
            let _ = file.read(&mut data)?;

            let asset = Arc::new(A::load(path, &data)?);

            self.assets.insert(path.to_owned(), asset.clone());
            Ok(asset)
        }
    }

    pub fn get_from_bytes(&mut self, path: &str, data: &[u8]) -> Result<Arc<A>> {
        appearance_profiling::profile_function!();

        if let Some(asset) = self.assets.get(path) {
            Ok(asset.clone())
        } else {
            let asset = Arc::new(A::load(path, data)?);

            self.assets.insert(path.to_owned(), asset.clone());
            Ok(asset)
        }
    }
}
