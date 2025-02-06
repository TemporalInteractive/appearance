use core::str;
use std::io::Write;

use glam::Mat4;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, bytemuck::NoUninit, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct CameraUpdateData {
    pub transform_matrix_bytes: Mat4,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub _padding: u32,
}

#[derive(Debug, Clone, Copy, bytemuck::NoUninit, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct SpawnModelData {
    pub transform_matrix: Mat4,
    pub entity_uuid: Uuid,
    asset_path_bytes: [u8; 256],
}

impl SpawnModelData {
    pub fn new(transform_matrix: Mat4, entity_uuid: Uuid, asset_path: &String) -> Self {
        let mut asset_path_bytes = [0u8; 256];
        {
            let mut asset_path_bytes = &mut asset_path_bytes[..];
            let _ = asset_path_bytes.write(asset_path.as_bytes()).unwrap();
        }

        Self {
            transform_matrix,
            entity_uuid,
            asset_path_bytes,
        }
    }

    pub fn asset_path(&self) -> &str {
        let nul_range_end = self
            .asset_path_bytes
            .iter()
            .position(|&c| c == b'\0')
            .unwrap_or(self.asset_path_bytes.len());

        str::from_utf8(&self.asset_path_bytes[0..nul_range_end]).unwrap()
    }
}

#[derive(Debug, Clone, Copy, bytemuck::NoUninit, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct TransformModelData {
    pub transform_matrix: Mat4,
    pub entity_uuid: Uuid,
}

#[derive(Debug, Clone, Copy, bytemuck::NoUninit, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct DestroyModelData {
    pub entity_uuid: Uuid,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Copy)]
pub enum VisibleWorldActionType {
    CameraUpdate(CameraUpdateData),
    SpawnModel(SpawnModelData),
    TransformModel(TransformModelData),
    DestroyModel(DestroyModelData),
    Clear(u32),
}

impl From<VisibleWorldActionType> for u32 {
    fn from(val: VisibleWorldActionType) -> Self {
        match val {
            VisibleWorldActionType::CameraUpdate(_) => 0,
            VisibleWorldActionType::SpawnModel(_) => 1,
            VisibleWorldActionType::TransformModel(_) => 2,
            VisibleWorldActionType::DestroyModel(_) => 3,
            VisibleWorldActionType::Clear(_) => 4,
        }
    }
}

impl VisibleWorldActionType {
    pub fn from_ty_and_bytes(ty: u32, bytes: &[u8]) -> Self {
        match ty {
            0 => Self::CameraUpdate(*bytemuck::from_bytes::<CameraUpdateData>(bytes)),
            1 => Self::SpawnModel(*bytemuck::from_bytes::<SpawnModelData>(bytes)),
            2 => Self::TransformModel(*bytemuck::from_bytes::<TransformModelData>(bytes)),
            3 => Self::DestroyModel(*bytemuck::from_bytes::<DestroyModelData>(bytes)),
            4 => Self::Clear(*bytemuck::from_bytes::<u32>(bytes)),
            _ => panic!(),
        }
    }

    pub fn data_size_from_ty(ty: u32) -> usize {
        match ty {
            0 => std::mem::size_of::<CameraUpdateData>(),
            1 => std::mem::size_of::<SpawnModelData>(),
            2 => std::mem::size_of::<TransformModelData>(),
            3 => std::mem::size_of::<DestroyModelData>(),
            4 => std::mem::size_of::<u32>(),
            _ => panic!(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            Self::CameraUpdate(data) => bytemuck::bytes_of(data),
            Self::SpawnModel(data) => bytemuck::bytes_of(data),
            Self::TransformModel(data) => bytemuck::bytes_of(data),
            Self::DestroyModel(data) => bytemuck::bytes_of(data),
            Self::Clear(data) => bytemuck::bytes_of(data),
        }
    }

    pub fn must_sync(&self) -> bool {
        match &self {
            Self::CameraUpdate(_) => false,
            Self::SpawnModel(_) => true,
            Self::TransformModel(_) => false,
            Self::DestroyModel(_) => true,
            Self::Clear(_) => true,
        }
    }
}

/// A visible action in the world, used to notify render nodes how the world changes
pub struct VisibleWorldAction {
    pub ty: u32,
    pub data: Vec<u8>,
    pub must_sync: bool,
}

impl VisibleWorldAction {
    pub fn new(action: VisibleWorldActionType) -> Self {
        let ty = action.into();
        let data = action.as_bytes().to_vec();

        Self {
            ty,
            data,
            must_sync: action.must_sync(),
        }
    }
}
