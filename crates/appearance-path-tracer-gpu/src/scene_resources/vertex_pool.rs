use appearance_wgpu::wgpu;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec4};

pub const MAX_VERTEX_POOL_VERTICES: usize = 1024 * 1024 * 32;

pub struct VertexPoolAlloc {
    pub slice: VertexPoolSlice,
    pub index: u32,
}

#[derive(Pod, Clone, Copy, Zeroable, PartialEq, Eq)]
#[repr(C)]
pub struct VertexPoolSlice {
    first_vertex: u32,
    num_vertices: u32,
    first_index: u32,
    num_indices: u32,
    pub material_idx: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
}

impl VertexPoolSlice {
    pub fn first_vertex(&self) -> u32 {
        self.first_vertex
    }

    pub fn first_index(&self) -> u32 {
        self.first_index
    }

    fn last_vertex(&self) -> u32 {
        self.first_vertex + self.num_vertices
    }

    fn last_index(&self) -> u32 {
        self.first_index + self.num_indices
    }
}

pub struct VertexPool {
    vertex_position_buffer: wgpu::Buffer,
    vertex_normal_buffer: wgpu::Buffer,
    vertex_tex_coord_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    slices_buffer: wgpu::Buffer,

    slices: Vec<VertexPoolSlice>,

    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl VertexPool {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_position_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool vertex_positions"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<Vec4>() * MAX_VERTEX_POOL_VERTICES) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_normal_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool vertex_normals"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<Vec4>() * MAX_VERTEX_POOL_VERTICES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let vertex_tex_coord_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool vertex_tex_coords"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<Vec2>() * MAX_VERTEX_POOL_VERTICES) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_VERTICES * 3) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let slices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool slices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<VertexPoolSlice>() * MAX_VERTEX_POOL_VERTICES / 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vertex_position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: vertex_normal_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: vertex_tex_coord_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: slices_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            vertex_position_buffer,
            vertex_normal_buffer,
            vertex_tex_coord_buffer,
            index_buffer,
            slices_buffer,
            slices: Vec::new(),
            bind_group_layout,
            bind_group,
        }
    }

    pub fn write_vertex_data(
        &self,
        vertex_positions: &[Vec4],
        vertex_normals: &[Vec4],
        vertex_tex_coords: &[Vec2],
        indices: &[u32],
        slice: VertexPoolSlice,
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            &self.vertex_position_buffer,
            (slice.first_vertex as usize * std::mem::size_of::<Vec4>()) as u64,
            bytemuck::cast_slice(vertex_positions),
        );
        queue.write_buffer(
            &self.vertex_normal_buffer,
            (slice.first_vertex as usize * std::mem::size_of::<Vec4>()) as u64,
            bytemuck::cast_slice(vertex_normals),
        );
        queue.write_buffer(
            &self.vertex_tex_coord_buffer,
            (slice.first_vertex as usize * std::mem::size_of::<Vec2>()) as u64,
            bytemuck::cast_slice(vertex_tex_coords),
        );

        queue.write_buffer(
            &self.index_buffer,
            (slice.first_index as usize * std::mem::size_of::<u32>()) as u64,
            bytemuck::cast_slice(indices),
        );
    }

    pub fn write_slices(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.slices_buffer,
            0,
            bytemuck::cast_slice(self.slices.as_slice()),
        );
    }

    pub fn alloc(&mut self, num_vertices: u32, num_indices: u32) -> VertexPoolAlloc {
        let first_vertex = self
            .first_available_vertex(num_vertices)
            .expect("Vertex pool ran out of vertices!");
        let first_index = self
            .first_available_index(num_indices)
            .expect("Vertex pool ran out of indices!");

        let slice = VertexPoolSlice {
            first_vertex,
            num_vertices,
            first_index,
            num_indices,
            material_idx: 0,
            _padding0: 0,
            _padding1: 0,
            _padding2: 0,
        };
        self.slices.push(slice);

        VertexPoolAlloc {
            slice,
            index: self.slices.len() as u32 - 1,
        }
    }

    pub fn free(_index: u32) {
        todo!()
    }

    fn first_available_vertex(&self, num_vertices: u32) -> Option<u32> {
        if self.slices.is_empty() && MAX_VERTEX_POOL_VERTICES as u32 > num_vertices {
            return Some(0);
        }

        for i in 0..self.slices.len() {
            let prev = if i > 0 {
                self.slices[i - 1].last_vertex()
            } else {
                0
            };

            let space = self.slices[i].first_vertex - prev;
            if space >= num_vertices {
                return Some(prev + num_vertices);
            }
        }

        let back = self.slices.last().unwrap().last_vertex();
        if back + num_vertices <= MAX_VERTEX_POOL_VERTICES as u32 {
            return Some(back);
        }

        None
    }

    fn first_available_index(&self, num_indices: u32) -> Option<u32> {
        if self.slices.is_empty() && MAX_VERTEX_POOL_VERTICES as u32 * 3 > num_indices {
            return Some(0);
        }

        for i in 0..self.slices.len() {
            let prev = if i > 0 {
                self.slices[i - 1].last_index()
            } else {
                0
            };

            let space = self.slices[i].first_index - prev;
            if space >= num_indices {
                return Some(prev + num_indices);
            }
        }

        let back = self.slices.last().unwrap().last_index();
        if back + num_indices <= MAX_VERTEX_POOL_VERTICES as u32 {
            return Some(back);
        }

        None
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn vertex_position_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_position_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }
}
