use crate::gpu_structs::Instance;
use crate::resources::Models;

pub struct GpuBuffer<T> {
    staging: Vec<T>,
    capacity_in_bytes: usize,
    buffer: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsage,
}

impl<T: Copy + bytemuck::Pod> GpuBuffer<T> {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsage) -> Self {
        let capacity_in_bytes = std::mem::size_of::<T>();

        Self {
            staging: Vec::with_capacity(1),
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: capacity_in_bytes as u64,
                usage: wgpu::BufferUsage::COPY_DST | usage,
                mapped_at_creation: false,
            }),
            label,
            usage,
            capacity_in_bytes,
        }
    }

    pub fn slice(&self) -> (wgpu::BufferSlice, u32) {
        (self.buffer.slice(..), self.staging.len() as u32)
    }

    pub fn clear(&mut self) {
        self.staging.clear();
    }

    pub fn stage(&mut self, slice: &[T]) {
        self.staging.extend_from_slice(slice);
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.staging.is_empty() {
            return;
        }

        let bytes = bytemuck::cast_slice(&self.staging);

        if self.capacity_in_bytes < bytes.len() {
            self.capacity_in_bytes = bytes.len().max(self.capacity_in_bytes * 2);

            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: self.capacity_in_bytes as u64,
                usage: wgpu::BufferUsage::COPY_DST | self.usage,
                mapped_at_creation: true,
            });

            self.buffer
                .slice(..)
                .get_mapped_range_mut()
                .copy_from_slice(bytes);
            self.buffer.unmap();
        } else {
            queue.write_buffer(&self.buffer, 0, bytes)
        }
    }
}

pub struct ShipBuffer {
    staging: [Vec<Instance>; Models::COUNT],
    buffer: wgpu::Buffer,
    capacity_in_bytes: usize,
}

impl ShipBuffer {
    const LABEL: &'static str = "ship instance buffer";

    pub fn new(device: &wgpu::Device) -> Self {
        let capacity_in_bytes = std::mem::size_of::<Instance>() * Models::COUNT;

        Self {
            staging: Default::default(),
            capacity_in_bytes,
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(Self::LABEL),
                size: capacity_in_bytes as u64,
                usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::VERTEX,
                mapped_at_creation: false,
            }),
        }
    }

    pub fn clear(&mut self) {
        for buffer in &mut self.staging {
            buffer.clear();
        }
    }

    pub fn slice(&self) -> (wgpu::BufferSlice, [u32; Models::COUNT]) {
        let mut lengths = [0; Models::COUNT];
        for i in 0..Models::COUNT {
            lengths[i] = self.staging[i].len() as u32;
        }

        (self.buffer.slice(..), lengths)
    }

    pub fn stage(&mut self, instance: Instance, ty: usize) {
        self.staging[ty].push(instance);
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let sum_length = self
            .staging
            .iter()
            .map(|buffer| buffer.len())
            .sum::<usize>()
            * std::mem::size_of::<Instance>();

        if sum_length == 0 {
            return;
        }

        if sum_length > self.capacity_in_bytes {
            self.capacity_in_bytes = sum_length.max(self.capacity_in_bytes * 2);

            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(Self::LABEL),
                size: self.capacity_in_bytes as u64,
                usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::VERTEX,
                mapped_at_creation: false,
            });
        }

        let mut offset = 0;

        for buffer in &self.staging {
            let bytes = bytemuck::cast_slice(buffer);
            queue.write_buffer(&self.buffer, offset, bytes);
            offset += bytes.len() as u64;
        }
    }
}
