// SPDX-License-Identifier: GPL-3.0-or-later

/// Screen orientation, same meaning as upstream `Orientation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Portrait,
    Landscape,
    ReversePortrait,
    ReverseLandscape,
}

/// Simple owned RGBA8 image (row-major, 4 bytes per pixel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

impl Rgba {
    pub fn new(width: usize, height: usize, data: Vec<u8>) -> Self {
        assert_eq!(data.len(), width * height * 4, "RGBA buffer size mismatch");
        Self { width, height, data }
    }

    pub fn solid(width: usize, height: usize, px: [u8; 4]) -> Self {
        let mut data = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            data.extend_from_slice(&px);
        }
        Self { width, height, data }
    }

    #[inline]
    fn px(&self, x: usize, y: usize) -> &[u8] {
        let i = (y * self.width + x) * 4;
        &self.data[i..i + 4]
    }

    /// Same as PIL `rotate(90, expand=True)` (counter-clockwise).
    pub fn rotate90(&self) -> Rgba {
        let (w, h) = (self.width, self.height);
        let mut out = Vec::with_capacity(w * h * 4);
        // new size: width = h, height = w ; new(x', y') = old(w-1-y', x')
        for yp in 0..w {
            for xp in 0..h {
                out.extend_from_slice(self.px(w - 1 - yp, xp));
            }
        }
        Rgba::new(h, w, out)
    }

    /// Same as PIL `rotate(270, expand=True)` (clockwise 90).
    pub fn rotate270(&self) -> Rgba {
        let (w, h) = (self.width, self.height);
        let mut out = Vec::with_capacity(w * h * 4);
        // new(x', y') = old(y', h-1-x')
        for yp in 0..w {
            for xp in 0..h {
                out.extend_from_slice(self.px(yp, h - 1 - xp));
            }
        }
        Rgba::new(h, w, out)
    }

    /// Same as PIL `rotate(180)`.
    pub fn rotate180(&self) -> Rgba {
        let (w, h) = (self.width, self.height);
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                out.extend_from_slice(self.px(w - 1 - x, h - 1 - y));
            }
        }
        Rgba::new(w, h, out)
    }

    /// Crop the top-left `w` x `h` region (PIL `crop((0,0,w,h))`).
    pub fn crop_top_left(&self, w: usize, h: usize) -> Rgba {
        let (w, h) = (w.min(self.width), h.min(self.height));
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            let start = y * self.width * 4;
            out.extend_from_slice(&self.data[start..start + w * 4]);
        }
        Rgba::new(w, h, out)
    }

    /// RGBA -> BGRA (4 bytes/pixel).
    pub fn to_bgra(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len());
        for p in self.data.chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
        out
    }

    /// RGBA -> BGR (3 bytes/pixel, alpha dropped).
    pub fn to_bgr(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height * 3);
        for p in self.data.chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        out
    }
}
