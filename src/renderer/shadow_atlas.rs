// shadow_atlas.rs

use std::cmp::{max, min};
use wgpu::{Device, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, Extent3d, Queue};
use std::sync::{Arc, RwLock};
use log::{info, warn};
use shipyard::Unique;
use crate::renderer::types::sampler::SamplerParameters;
use crate::renderer::types::texture::Texture;

/// Represents a rectangle region in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Data returned when a tile is allocated.
#[derive(Debug, Clone, Copy)]
pub struct AtlasTile {
    /// The allocated rectangle in pixels.
    pub rect: Rect,
    /// The UV offset (in [0,1]) to the top-left corner.
    pub uv_offset: glam::Vec2,
    /// The UV scale (in [0,1]) representing the size of the tile.
    pub uv_scale: glam::Vec2,
}

impl AtlasTile {
    /// Useful suballocation to further split a tile into smaller tiles.
    /// This is useful for cascaded shadow maps, for example. Or point light shadow maps.
    /// It returns a vector of tiles that are suballocated from the current tile.
    ///
    /// * num_tiles_x - The number of tiles to split the current tile into horizontally.
    /// * num_tiles_y - The number of tiles to split the current tile into vertically.
    pub fn suballocate(&self, num_tiles_x: u32, num_tiles_y: u32) -> Vec<AtlasTile> {
        let mut tiles = Vec::new();
        let tile_width = self.rect.width / num_tiles_x;
        let tile_height = self.rect.height / num_tiles_y;
        for y in 0..num_tiles_y {
            for x in 0..num_tiles_x {
                let tile_rect = Rect {
                    x: self.rect.x + x * tile_width,
                    y: self.rect.y + y * tile_height,
                    width: tile_width,
                    height: tile_height,
                };
                let uv_offset = glam::Vec2::new(tile_rect.x as f32 / self.rect.width as f32, tile_rect.y as f32 / self.rect.height as f32);
                let uv_scale = glam::Vec2::new(tile_rect.width as f32 / self.rect.width as f32, tile_rect.height as f32 / self.rect.height as f32);
                tiles.push(AtlasTile { rect: tile_rect, uv_offset, uv_scale });
            }
        }
        tiles
    }
}

/// A simple shadow atlas that supports dynamic tile allocation.
#[derive(Unique)]
pub struct ShadowAtlas {
    pub texture: Arc<Texture>,
    pub shadow_sampler: SamplerParameters,
    pub width: u32,
    pub height: u32,
    // Free rectangles in the atlas (initially just one: the full atlas).
    free_rects: Vec<Rect>,
    // Allocated tiles.
    tiles: Vec<Arc<RwLock<AtlasTile>>>,
}

impl ShadowAtlas {
    /// Create a new shadow atlas with the given dimensions and texture format.
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32, format: TextureFormat) -> Self {
        // Create a screen texture for the atlas.
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Atlas Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        info!("Created ShadowAtlas with size {}x{}", width, height);
        let sampler = SamplerParameters {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 100.0,
            compare: Some(wgpu::CompareFunction::Less),
            anisotropy_clamp: 1,
            border_color: None,
        };

        Self {
            texture: Arc::new(Texture::new_screen_texture(device, queue, (width, height), format, false)),
            shadow_sampler: sampler,
            width,
            height,
            free_rects: vec![Rect { x: 0, y: 0, width, height }],
            tiles: Vec::new(),
        }
    }

    /// Attempts to allocate a tile of the given width and height (in pixels) in the atlas.
    /// Returns Some(AtlasTile) if successful, or None if there isn’t enough space.
    pub fn allocate_tile(&mut self, req_width: u32, req_height: u32) -> Option<Arc<RwLock<AtlasTile>>> {
        // Simple first-fit allocation: find the first free rectangle that fits the requested size.
        let mut allocated_rect = None;
        let mut free_rect_index = None;
        for (i, rect) in self.free_rects.iter().enumerate() {
            if rect.width >= req_width && rect.height >= req_height {
                allocated_rect = Some(Rect {
                    x: rect.x,
                    y: rect.y,
                    width: req_width,
                    height: req_height,
                });
                free_rect_index = Some(i);
                break;
            }
        }

        if let (Some(tile_rect), Some(idx)) = (allocated_rect, free_rect_index) {
            // Remove the free rectangle and split it into up to two new free rectangles.
            let free_rect = self.free_rects.swap_remove(idx);
            // Right remaining rectangle.
            if free_rect.width > req_width {
                self.free_rects.push(Rect {
                    x: free_rect.x + req_width,
                    y: free_rect.y,
                    width: free_rect.width - req_width,
                    height: req_height,
                });
            }
            // Bottom remaining rectangle.
            if free_rect.height > req_height {
                self.free_rects.push(Rect {
                    x: free_rect.x,
                    y: free_rect.y + req_height,
                    width: free_rect.width,
                    height: free_rect.height - req_height,
                });
            }
            // Optionally, merge adjacent free rectangles to optimize space.
            self.optimize_free_rects();
            // Compute UV offset/scale.
            let uv_offset = glam::Vec2::new(tile_rect.x as f32 / self.width as f32, tile_rect.y as f32 / self.height as f32);
            let uv_scale = glam::Vec2::new(tile_rect.width as f32 / self.width as f32, tile_rect.height as f32 / self.height as f32);
            info!("Allocated tile of size {}x{} at ({}, {}) in ShadowAtlas", req_width, req_height, tile_rect.x, tile_rect.y);
            info!("UV offset: {:?}, UV scale: {:?}", uv_offset, uv_scale);
            // Create the tile and store it.
            let tile = AtlasTile { rect: tile_rect, uv_offset, uv_scale };
            let tile = Arc::new(RwLock::new(tile));
            self.tiles.push(tile.clone());
            Some(tile)
        } else {
            warn!("Failed to allocate tile of size {}x{} in ShadowAtlas", req_width, req_height);
            None
        }
    }

    /// Attempts to merge adjacent free rectangles.
    /// This simple algorithm looks for rectangles that share an edge and merges them.
    pub fn optimize_free_rects(&mut self) {
        let mut i = 0;
        while i < self.free_rects.len() {
            let mut j = i + 1;
            while j < self.free_rects.len() {
                let rect_a = self.free_rects[i];
                let rect_b = self.free_rects[j];
                // Check if they are adjacent horizontally.
                if rect_a.y == rect_b.y && rect_a.height == rect_b.height {
                    if rect_a.x + rect_a.width == rect_b.x {
                        // Merge rect_a and rect_b horizontally.
                        self.free_rects[i] = Rect {
                            x: rect_a.x,
                            y: rect_a.y,
                            width: rect_a.width + rect_b.width,
                            height: rect_a.height,
                        };
                        self.free_rects.remove(j);
                        continue;
                    } else if rect_b.x + rect_b.width == rect_a.x {
                        // Merge rect_b and rect_a horizontally.
                        self.free_rects[i] = Rect {
                            x: rect_b.x,
                            y: rect_b.y,
                            width: rect_a.width + rect_b.width,
                            height: rect_a.height,
                        };
                        self.free_rects.remove(j);
                        continue;
                    }
                }
                // Check if they are adjacent vertically.
                if rect_a.x == rect_b.x && rect_a.width == rect_b.width {
                    if rect_a.y + rect_a.height == rect_b.y {
                        self.free_rects[i] = Rect {
                            x: rect_a.x,
                            y: rect_a.y,
                            width: rect_a.width,
                            height: rect_a.height + rect_b.height,
                        };
                        self.free_rects.remove(j);
                        continue;
                    } else if rect_b.y + rect_b.height == rect_a.y {
                        self.free_rects[i] = Rect {
                            x: rect_b.x,
                            y: rect_b.y,
                            width: rect_a.width,
                            height: rect_a.height + rect_b.height,
                        };
                        self.free_rects.remove(j);
                        continue;
                    }
                }
                j += 1;
            }
            i += 1;
        }
    }
}
