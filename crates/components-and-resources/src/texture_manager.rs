#[derive(Default)]
pub struct TextureManager {
    texture_views: Vec<wgpu::TextureView>,
}

impl TextureManager {
    pub fn add(&mut self, texture: wgpu::TextureView) -> u32 {
        let index = self.texture_views.len() as u32;
        self.texture_views.push(texture);
        index
    }

    pub fn into_bind_group(
        self,
        device: &wgpu::Device,
        sampler: &wgpu::Sampler,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("merged textures bind group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureViewArray(
                        &self.texture_views.iter().collect::<Vec<_>>(),
                    ),
                },
            ],
        })
    }

    pub fn count(&self) -> u32 {
        self.texture_views.len() as u32
    }
}
