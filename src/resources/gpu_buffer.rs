pub struct GpuBuffer<T> {
    staging: Vec<T>,
    capacity_in_bytes: usize,
    buffer: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsage,
}

impl<T: Copy + bytemuck::Pod> GpuBuffer<T> {
    pub fn new(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsage,
        initial_capacity: usize,
    ) -> Self {
        let capacity_in_bytes = initial_capacity * std::mem::size_of::<T>();

        Self {
            staging: Vec::with_capacity(initial_capacity),
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
