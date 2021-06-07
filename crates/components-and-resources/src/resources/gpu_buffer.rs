use crate::gpu_structs::{DrawIndexedIndirect, Instance};
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
                .slice(..bytes.len() as u64)
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
    draw_indirect_buffer: wgpu::Buffer,
    draw_indirect_count: u32,
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
            draw_indirect_buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("draw indirect buffer"),
                size: (std::mem::size_of::<DrawIndexedIndirect>() * Models::COUNT) as u64,
                usage: wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::INDIRECT,
                mapped_at_creation: false,
            }),
            draw_indirect_count: 0,
        }
    }

    pub fn clear(&mut self) {
        for buffer in &mut self.staging {
            buffer.clear();
        }
    }

    pub fn slice(&self) -> (wgpu::BufferSlice, [u32; Models::COUNT], &wgpu::Buffer, u32) {
        let mut lengths = [0; Models::COUNT];
        #[allow(clippy::needless_range_loop)]
        for i in 0..Models::COUNT {
            lengths[i] = self.staging[i].len() as u32;
        }

        (
            self.buffer.slice(..),
            lengths,
            &self.draw_indirect_buffer,
            self.draw_indirect_count,
        )
    }

    pub fn stage(&mut self, instance: Instance, ty: usize) {
        self.staging[ty].push(instance);
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, models: &Models) {
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

        let mut draw_indirect_array = [DrawIndexedIndirect::default(); Models::COUNT];
        let mut draw_indirect_offset = 0;
        let mut instance_offset = 0;
        let mut index_offset = 0;

        for i in 0..Models::COUNT {
            let buffer = &self.staging[i];

            if !buffer.is_empty() {
                let bytes = bytemuck::cast_slice(buffer);
                queue.write_buffer(&self.buffer, offset, bytes);
                offset += bytes.len() as u64;

                let instance_count = buffer.len() as u32;
                let index_count = models.models[i].num_indices;

                draw_indirect_array[draw_indirect_offset] = DrawIndexedIndirect {
                    vertex_offset: 0,
                    base_instance: instance_offset,
                    instance_count,
                    base_index: index_offset,
                    index_count,
                };

                draw_indirect_offset += 1;
                instance_offset += instance_count;
                index_offset += index_count;
            }
        }

        self.draw_indirect_count = draw_indirect_offset as u32;
        queue.write_buffer(
            &self.draw_indirect_buffer,
            0,
            bytemuck::cast_slice(&draw_indirect_array[..draw_indirect_offset]),
        );
    }
}
