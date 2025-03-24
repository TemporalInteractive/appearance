use appearance_model::mesh::PackedVertex;
use appearance_wgpu::wgpu::{self, util::DeviceExt};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;

pub const MAX_VERTEX_POOL_VERTICES: usize = 1024 * 1024 * 32;

pub struct VertexPoolWriteData<'a> {
    pub packed_vertices: &'a [PackedVertex],
    pub indices: &'a [u32],
    pub triangle_material_indices: &'a [u32],
}

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

    pub fn last_index(&self) -> u32 {
        self.first_index + self.num_indices
    }
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct VertexPoolConstants {
    num_emissive_triangle_instances: u32,
    num_emissive_triangles: u32,
    _padding1: u32,
    _padding2: u32,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct EmissiveTriangleInstance {
    transform: Mat4,
    vertex_pool_slice_idx: u32,
    num_triangles: u32,
    _padding0: u32,
    _padding1: u32,
}

#[derive(Pod, Clone, Copy, Zeroable)]
#[repr(C)]
struct BlasInstance {
    emissive_blas_instance_idx: u32,
    vertex_pool_slice_index: u32,
    _padding0: u32,
    _padding1: u32,
}

pub struct VertexPool {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    triangle_material_index_buffer: wgpu::Buffer,
    emissive_triangle_instance_buffer: wgpu::Buffer,
    emissive_triangle_instance_cdf_buffer: wgpu::Buffer,
    blas_instances_buffer: wgpu::Buffer,
    slices_buffer: wgpu::Buffer,

    emissive_triangle_instances: Vec<EmissiveTriangleInstance>,
    emissive_triangle_count: u32,
    blas_instances: Vec<BlasInstance>,
    slices: Vec<VertexPoolSlice>,

    bind_group_layout: wgpu::BindGroupLayout,
}

impl VertexPool {
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool vertices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<PackedVertex>() * MAX_VERTEX_POOL_VERTICES) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_VERTICES * 3) as u64,
            usage: wgpu::BufferUsages::BLAS_INPUT
                | wgpu::BufferUsages::INDEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST,
        });

        let triangle_material_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool triangle_material_indices"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<u32>() * MAX_VERTEX_POOL_VERTICES / 3) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let emissive_triangle_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool emissive_triangle_instances"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<EmissiveTriangleInstance>() * 1024) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let emissive_triangle_instance_cdf_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(
                "appearance-path-tracer-gpu::vertex_pool emissive_triangle_instance_cdf_buffer",
            ),
            mapped_at_creation: false,
            size: (std::mem::size_of::<f32>() * 1024) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let blas_instances_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool blas_instances"),
            mapped_at_creation: false,
            size: (std::mem::size_of::<BlasInstance>() * 1024) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
                        ty: wgpu::BufferBindingType::Uniform,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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

        Self {
            vertex_buffer,
            index_buffer,
            triangle_material_index_buffer,
            emissive_triangle_instance_buffer,
            emissive_triangle_instance_cdf_buffer,
            blas_instances_buffer,
            slices_buffer,
            emissive_triangle_instances: Vec::new(),
            emissive_triangle_count: 0,
            blas_instances: Vec::new(),
            slices: Vec::new(),
            bind_group_layout,
        }
    }

    pub fn write_vertex_data(
        &self,
        data: &VertexPoolWriteData,
        slice: VertexPoolSlice,
        queue: &wgpu::Queue,
    ) {
        queue.write_buffer(
            &self.vertex_buffer,
            (slice.first_vertex as usize * std::mem::size_of::<PackedVertex>()) as u64,
            bytemuck::cast_slice(data.packed_vertices),
        );

        queue.write_buffer(
            &self.index_buffer,
            (slice.first_index as usize * std::mem::size_of::<u32>()) as u64,
            bytemuck::cast_slice(data.indices),
        );

        queue.write_buffer(
            &self.triangle_material_index_buffer,
            (slice.first_index as usize / 3 * std::mem::size_of::<u32>()) as u64,
            bytemuck::cast_slice(data.triangle_material_indices),
        );
    }

    pub fn write_slices(&mut self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.slices_buffer,
            0,
            bytemuck::cast_slice(self.slices.as_slice()),
        );

        queue.write_buffer(
            &self.emissive_triangle_instance_buffer,
            0,
            bytemuck::cast_slice(self.emissive_triangle_instances.as_slice()),
        );

        queue.write_buffer(
            &self.emissive_triangle_instance_cdf_buffer,
            0,
            bytemuck::cast_slice(self.calculate_emissive_slice_instance_cdfs().as_slice()),
        );

        queue.write_buffer(
            &self.blas_instances_buffer,
            0,
            bytemuck::cast_slice(self.blas_instances.as_slice()),
        );
    }

    pub fn submit_slice_instance(&mut self, index: u32, transform: Mat4, is_emissive: bool) {
        let mut emissive_blas_instance_idx = u32::MAX;

        if is_emissive {
            let num_triangles = self.slices[index as usize].num_indices / 3;
            let instance = EmissiveTriangleInstance {
                transform,
                vertex_pool_slice_idx: index,
                num_triangles,
                _padding0: 0,
                _padding1: 0,
            };
            self.emissive_triangle_instances.push(instance);
            self.emissive_triangle_count += num_triangles;

            emissive_blas_instance_idx = self.emissive_triangle_instances.len() as u32 - 1;
        }

        let instance = BlasInstance {
            emissive_blas_instance_idx,
            vertex_pool_slice_index: index,
            _padding0: 0,
            _padding1: 0,
        };
        self.blas_instances.push(instance);
    }

    fn calculate_emissive_slice_instance_cdfs(&self) -> Vec<f32> {
        let mut cdfs = vec![];

        let mut cdf = 0.0;
        for instance in &self.emissive_triangle_instances {
            let pdf = instance.num_triangles as f32 / self.emissive_triangle_count as f32;
            cdf += pdf;
            cdfs.push(cdf);
        }

        cdfs
    }

    pub fn alloc(
        &mut self,
        num_vertices: u32,
        num_indices: u32,
        material_idx: u32,
    ) -> VertexPoolAlloc {
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
            material_idx,
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

    pub fn _free(_index: u32) {
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

    pub fn bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        let constants = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("appearance-path-tracer-gpu::vertex_pool constants"),
            contents: bytemuck::bytes_of(&VertexPoolConstants {
                num_emissive_triangle_instances: self.emissive_triangle_instances.len() as u32,
                num_emissive_triangles: self.emissive_triangle_count,
                _padding1: 0,
                _padding2: 0,
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: constants.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.triangle_material_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.slices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.emissive_triangle_instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self
                        .emissive_triangle_instance_cdf_buffer
                        .as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: self.blas_instances_buffer.as_entire_binding(),
                },
            ],
        })
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }

    pub fn end_frame(&mut self) {
        self.emissive_triangle_instances.clear();
        self.emissive_triangle_count = 0;
        self.blas_instances.clear();
    }
}
