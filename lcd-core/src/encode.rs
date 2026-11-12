// SPDX-License-Identifier: GPL-3.0-or-later
//
// Frame encoders for the 2.1" (480x480) Rev C screen, mirroring the upstream
// `_generate_full_image` / `_generate_update_image` code paths for
// SubRevision.REV_2INCH.

use crate::image::{Orientation, Rgba};
use crate::protocol::*;

pub const WIDTH: usize = 480;
pub const HEIGHT: usize = 480;

/// Rotate for a full-frame transfer (non-8" screens).
fn rotate_for_full(img: &Rgba, o: Orientation) -> Rgba {
    match o {
        Orientation::Portrait => img.rotate90(),
        Orientation::ReversePortrait => img.rotate270(),
        Orientation::ReverseLandscape => img.rotate180(),
        Orientation::Landscape => img.clone(),
    }
}

/// Bytes of the full-frame payload (BGRA, chunked, *not yet padded*).
pub fn full_image_payload(img: &Rgba, o: Orientation) -> Vec<u8> {
    let rotated = rotate_for_full(img, o);
    chunk_join(&rotated.to_bgra())
}

/// The ordered list of writes for a full-frame refresh.
/// Each item is `(bytes_to_write, read_len_after)`; `read_len_after == 0` means no read.
pub fn full_frame_sequence(img: &Rgba, o: Orientation) -> Vec<(Vec<u8>, usize)> {
    let size_field = ((WIDTH * WIDTH / 64) as u16).to_be_bytes();
    vec![
        (build_command(PRE_UPDATE_BITMAP, &[], PAD_NULL), 0),
        (build_command(START_DISPLAY_BITMAP, &[], PAD_START_DISPLAY_BITMAP), 0),
        (build_command(DISPLAY_BITMAP_2INCH, &size_field, PAD_NULL), 0),
        (
            pad_message(full_image_payload(img, o), PAD_NULL),
            STATUS_REPLY_LEN,
        ),
        (build_command(QUERY_STATUS, &[], PAD_NULL), STATUS_REPLY_LEN),
    ]
}

/// Result of encoding a partial update: the header (`pyd`) and the pixel data (`img`).
pub struct UpdateImage {
    pub header: Vec<u8>,
    pub data: Vec<u8>,
}

/// Encode a partial update at (x, y). Mirrors upstream for REV_2INCH
/// (3-byte BGR pixels). `count` is the running render counter.
pub fn update_image(img: &Rgba, x: usize, y: usize, count: u32, o: Orientation) -> UpdateImage {
    let (mut x0, mut y0) = (x as i64, y as i64);
    let mut image = img.clone();
    let (w_disp, h_disp) = (WIDTH as i64, HEIGHT as i64);

    match o {
        Orientation::Portrait => {
            image = image.rotate90();
            x0 = w_disp - x as i64 - image.height as i64;
        }
        Orientation::ReversePortrait => {
            image = image.rotate270();
            y0 = h_disp - y as i64 - image.width as i64;
        }
        Orientation::ReverseLandscape => {
            image = image.rotate180();
            y0 = w_disp - x as i64 - image.width as i64;
            x0 = h_disp - y as i64 - image.height as i64;
        }
        Orientation::Landscape => {
            x0 = y as i64;
            y0 = x as i64;
        }
    }

    let bgr = image.to_bgr();
    let row_bytes = image.width * 3;
    let mut raw: Vec<u8> = Vec::with_capacity(bgr.len() + image.height * 5);
    for (h, line) in bgr.chunks(row_bytes).enumerate() {
        let addr = ((x0 + h as i64) * h_disp + y0) as u32;
        raw.extend_from_slice(&addr.to_be_bytes()[1..4]); // 3 bytes big-endian
        raw.extend_from_slice(&(image.width as u16).to_be_bytes());
        raw.extend_from_slice(line);
    }

    let size = ((raw.len() + 2) as u32).to_be_bytes();
    let mut header = Vec::new();
    header.extend_from_slice(UPDATE_BITMAP);
    header.extend_from_slice(&size[1..4]);
    header.extend_from_slice(&[0, 0, 0]);
    header.extend_from_slice(&count.to_be_bytes());

    let mut data = if raw.len() > 250 { chunk_join(&raw) } else { raw };
    data.extend_from_slice(&[0xef, 0x69]);

    UpdateImage { header, data }
}

/// Ordered writes for a partial update (header, data, status query).
pub fn update_sequence(img: &Rgba, x: usize, y: usize, count: u32, o: Orientation) -> Vec<(Vec<u8>, usize)> {
    let u = update_image(img, x, y, count, o);
    vec![
        (pad_message(u.header, PAD_NULL), 0),
        (pad_message(u.data, PAD_NULL), 0),
        (build_command(QUERY_STATUS, &[], PAD_NULL), STATUS_REPLY_LEN),
    ]
}
