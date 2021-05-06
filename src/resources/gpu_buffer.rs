use wgpu::util::DeviceExt;

pub struct GpuBuffer<T> {
    staging: Vec<T>,
    buffer: wgpu::Buffer,
    label: &'static str,
    usage: wgpu::BufferUsage,
}

impl<T: Copy + bytemuck::Pod> GpuBuffer<T> {
    pub fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsage) -> Self {
        Self {
            staging: Vec::new(),
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: 0,
                usage: wgpu::BufferUsage::COPY_DST | usage,
                mapped_at_creation: false,
            }),
            label,
            usage,
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

    pub fn upload(&mut self, device: &wgpu::Device) {
        if self.staging.is_empty() {
            return;
        }

        self.buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(self.label),
            contents: bytemuck::cast_slice(&self.staging),
            usage: wgpu::BufferUsage::COPY_DST | self.usage,
        });
    }
}
