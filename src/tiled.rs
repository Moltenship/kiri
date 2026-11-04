use std::sync::Arc;

use gpui_kit::RenderImage;
use image::{Frame, RgbaImage};

/// GPUI uploads each image into a Metal atlas capped at 16384 px a side, so
/// large captures (scrolling, multi-display) must be split before upload.
pub const TILE: u32 = 4096;

/// A BGRA image stored only as GPU-ready tiles: the tiles are the single copy
/// of the pixels, so the editor never holds the image twice in RAM.
#[derive(Clone)]
pub struct TiledImage {
    pub width: u32,
    pub height: u32,
    cols: u32,
    tiles: Vec<Arc<RenderImage>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PxRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PxRect {
    pub fn clamp_to(self, width: u32, height: u32) -> Option<PxRect> {
        self.intersect(PxRect { x: 0, y: 0, w: width, h: height })
    }

    fn intersect(self, other: PxRect) -> Option<PxRect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = (self.x + self.w).min(other.x + other.w);
        let y1 = (self.y + self.h).min(other.y + other.h);
        (x1 > x0 && y1 > y0).then(|| PxRect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        })
    }
}

impl TiledImage {
    /// `bgra` is tightly packed unless `stride` says otherwise.
    pub fn from_bgra(width: u32, height: u32, bgra: &[u8], stride: usize) -> Self {
        let cols = width.div_ceil(TILE);
        let rows = height.div_ceil(TILE);
        let mut tiles = Vec::with_capacity((cols * rows) as usize);
        for row in 0..rows {
            for col in 0..cols {
                let rect = tile_rect(width, height, col, row);
                tiles.push(Arc::new(make_tile(rect, bgra, stride)));
            }
        }
        Self {
            width,
            height,
            cols,
            tiles,
        }
    }

    /// True when both share every tile, which is how unchanged snapshots look.
    pub fn same_pixels(&self, other: &TiledImage) -> bool {
        self.tiles.len() == other.tiles.len()
            && self.tiles.iter().zip(&other.tiles).all(|(a, b)| Arc::ptr_eq(a, b))
    }

    pub fn full(&self) -> PxRect {
        PxRect {
            x: 0,
            y: 0,
            w: self.width,
            h: self.height,
        }
    }

    pub fn tiles(&self) -> impl Iterator<Item = (PxRect, &Arc<RenderImage>)> {
        self.tiles.iter().enumerate().map(|(i, tile)| {
            let (col, row) = (i as u32 % self.cols, i as u32 / self.cols);
            (tile_rect(self.width, self.height, col, row), tile)
        })
    }

    /// Copies `rect` out as tightly packed BGRA.
    pub fn read(&self, rect: PxRect) -> Vec<u8> {
        let mut out = vec![0u8; (rect.w * rect.h * 4) as usize];
        let out_stride = (rect.w * 4) as usize;
        for (tile_rect, tile) in self.tiles() {
            let Some(part) = rect.intersect(tile_rect) else {
                continue;
            };
            let bytes = tile.as_bytes(0).expect("tile has one frame");
            let tile_stride = (tile_rect.w * 4) as usize;
            for y in 0..part.h {
                let src = ((part.y - tile_rect.y + y) as usize) * tile_stride
                    + ((part.x - tile_rect.x) * 4) as usize;
                let dst = ((part.y - rect.y + y) as usize) * out_stride
                    + ((part.x - rect.x) * 4) as usize;
                let len = (part.w * 4) as usize;
                out[dst..dst + len].copy_from_slice(&bytes[src..src + len]);
            }
        }
        out
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        (x < self.width && y < self.height).then(|| {
            let px = self.read(PxRect { x, y, w: 1, h: 1 });
            [px[0], px[1], px[2], px[3]]
        })
    }

    /// Writes tightly packed BGRA into `rect` and rebuilds only the touched tiles.
    /// Returns the replaced tiles so the caller can evict them from the GPU atlas.
    pub fn write(&mut self, rect: PxRect, bgra: &[u8]) -> Vec<Arc<RenderImage>> {
        let in_stride = (rect.w * 4) as usize;
        let mut replaced = Vec::new();
        for i in 0..self.tiles.len() {
            let (col, row) = (i as u32 % self.cols, i as u32 / self.cols);
            let tile_rect = tile_rect(self.width, self.height, col, row);
            let Some(part) = rect.intersect(tile_rect) else {
                continue;
            };
            let mut bytes = self.tiles[i]
                .as_bytes(0)
                .expect("tile has one frame")
                .to_vec();
            let tile_stride = (tile_rect.w * 4) as usize;
            for y in 0..part.h {
                let dst = ((part.y - tile_rect.y + y) as usize) * tile_stride
                    + ((part.x - tile_rect.x) * 4) as usize;
                let src =
                    ((part.y - rect.y + y) as usize) * in_stride + ((part.x - rect.x) * 4) as usize;
                let len = (part.w * 4) as usize;
                bytes[dst..dst + len].copy_from_slice(&bgra[src..src + len]);
            }
            let fresh = Arc::new(frame_image(tile_rect.w, tile_rect.h, bytes));
            replaced.push(std::mem::replace(&mut self.tiles[i], fresh));
        }
        replaced
    }
}

fn tile_rect(width: u32, height: u32, col: u32, row: u32) -> PxRect {
    let (x, y) = (col * TILE, row * TILE);
    PxRect {
        x,
        y,
        w: TILE.min(width - x),
        h: TILE.min(height - y),
    }
}

fn make_tile(rect: PxRect, bgra: &[u8], stride: usize) -> RenderImage {
    let row_len = (rect.w * 4) as usize;
    let mut bytes = Vec::with_capacity(row_len * rect.h as usize);
    for y in rect.y..rect.y + rect.h {
        let start = y as usize * stride + (rect.x * 4) as usize;
        bytes.extend_from_slice(&bgra[start..start + row_len]);
    }
    frame_image(rect.w, rect.h, bytes)
}

pub fn frame_image(w: u32, h: u32, bgra: Vec<u8>) -> RenderImage {
    // GPUI reads the frame buffer as BGRA regardless of the `Rgba` type name.
    let buffer = RgbaImage::from_raw(w, h, bgra).expect("buffer matches size");
    RenderImage::new([Frame::new(buffer)])
}
