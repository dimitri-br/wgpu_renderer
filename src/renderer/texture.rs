use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Texture{
    pub texture: Arc<wgpu::Texture>,
    pub view: Arc<wgpu::TextureView>,
}

impl Texture{
    pub fn load_from_bytes(device: &Arc<wgpu::Device>, queue: &wgpu::Queue, bytes: &[u8]) -> Self{
        let img = image::load_from_memory(bytes).unwrap().to_rgba8();
        let dimensions = img.dimensions();
        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo{
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self{
            texture: Arc::new(texture),
            view: Arc::new(view),
        }
    }

    pub fn load_from_file(device: &Arc<wgpu::Device>, queue: &wgpu::Queue, path: &Path) -> Self {
        let bytes = std::fs::read(path).unwrap();
        Self::load_from_bytes(device, queue, &bytes)
    }
}