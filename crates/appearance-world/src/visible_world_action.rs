use glam::Mat4;

#[derive(Debug, Clone, Copy, bytemuck::NoUninit, bytemuck::AnyBitPattern)]
#[repr(C)]
pub struct CameraUpdateData {
    pub transform_matrix_bytes: Mat4,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub _padding: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum VisibleWorldActionType {
    CameraUpdate(CameraUpdateData),
}

impl From<VisibleWorldActionType> for u32 {
    fn from(val: VisibleWorldActionType) -> Self {
        match val {
            VisibleWorldActionType::CameraUpdate(_) => 0,
        }
    }
}

impl VisibleWorldActionType {
    pub fn from_ty_and_bytes(ty: u32, bytes: &[u8]) -> Self {
        match ty {
            0 => Self::CameraUpdate(*bytemuck::from_bytes::<CameraUpdateData>(bytes)),
            _ => panic!(),
        }
    }

    pub fn data_size_from_ty(ty: u32) -> usize {
        match ty {
            0 => std::mem::size_of::<CameraUpdateData>(),
            _ => panic!(),
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            Self::CameraUpdate(data) => bytemuck::bytes_of(data),
        }
    }
}

/// A visible action in the world, used to notify render nodes how the world changes
pub struct VisibleWorldAction {
    pub ty: u32,
    pub data: Vec<u8>,
}

impl VisibleWorldAction {
    pub fn new(action: VisibleWorldActionType) -> Self {
        let ty = action.into();
        let data = action.as_bytes().to_vec();

        Self { ty, data }
    }
}
