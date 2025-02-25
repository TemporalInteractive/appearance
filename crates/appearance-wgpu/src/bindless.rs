use std::num::NonZeroU32;
use std::sync::mpsc::{channel, Receiver, Sender};

use wgpu::util::DeviceExt;

pub const MAX_BINDLESS_STORAGE_BUFFERS: usize = 1024;

#[derive(Debug)]
pub struct BindlessBuffer {
    id: u32,
    drop_sender: Sender<u32>,
}

impl BindlessBuffer {
    pub fn binding(&self) -> u32 {
        self.id
    }
}

impl Drop for BindlessBuffer {
    fn drop(&mut self) {
        self.drop_sender.send(self.id).unwrap();
    }
}

pub struct Bindless {
    empty_storage_buffer: wgpu::Buffer,
    storage_buffers: [Option<wgpu::Buffer>; MAX_BINDLESS_STORAGE_BUFFERS],

    drop_sender: Sender<u32>,
    drop_receiver: Receiver<u32>,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl Bindless {
    pub fn new(device: &wgpu::Device) -> Self {
        let empty_storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-wgpu::bindless_empty"),
            size: 4 * 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::all(),
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: Some(NonZeroU32::new(MAX_BINDLESS_STORAGE_BUFFERS as u32).unwrap()),
            }],
        });

        let storage_buffers = std::array::from_fn(|_| None);

        let (drop_sender, drop_receiver) = channel();

        Self {
            empty_storage_buffer,
            storage_buffers,
            drop_sender,
            drop_receiver,
            bind_group_layout,
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let mut storage_buffer_bindings = vec![];
        for storage_buffer in &self.storage_buffers {
            if let Some(buffer) = storage_buffer {
                storage_buffer_bindings.push(buffer.as_entire_buffer_binding());
            } else {
                storage_buffer_bindings.push(self.empty_storage_buffer.as_entire_buffer_binding());
            }
        }

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::BufferArray(&storage_buffer_bindings),
            }],
        })
    }

    pub fn create_storage_buffer(
        &mut self,
        desc: &wgpu::BufferDescriptor<'_>,
        device: &wgpu::Device,
    ) -> BindlessBuffer {
        let storage_buffer = device.create_buffer(desc);

        let i = self
            .storage_buffers
            .iter()
            .position(|i| i.is_none())
            .expect("Ran out of storage buffers!");
        self.storage_buffers[i] = Some(storage_buffer);

        BindlessBuffer {
            id: i as u32,
            drop_sender: self.drop_sender.clone(),
        }
    }

    pub fn create_storage_buffer_init(
        &mut self,
        desc: &wgpu::util::BufferInitDescriptor<'_>,
        device: &wgpu::Device,
    ) -> BindlessBuffer {
        let storage_buffer = device.create_buffer_init(desc);

        let i = self
            .storage_buffers
            .iter()
            .position(|i| i.is_none())
            .expect("Ran out of storage buffers!");
        self.storage_buffers[i] = Some(storage_buffer);

        BindlessBuffer {
            id: i as u32,
            drop_sender: self.drop_sender.clone(),
        }
    }

    pub fn get_storage_buffer(&self, bindless_buffer: &BindlessBuffer) -> &wgpu::Buffer {
        self.storage_buffers[bindless_buffer.id as usize]
            .as_ref()
            .unwrap()
    }

    pub fn update(&mut self) {
        for drop in &self.drop_receiver {
            self.storage_buffers[drop as usize] = None;
        }
    }
}
