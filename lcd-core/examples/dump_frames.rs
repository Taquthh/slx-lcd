// SPDX-License-Identifier: GPL-3.0-or-later
// Writes the exact byte stream for deterministic test frames, for parity checks with Python upstream.
use lcd_core::encode::{full_frame_sequence, update_sequence};
use lcd_core::protocol::{set_brightness, set_options};
use lcd_core::{Orientation, Rgba};
use std::{env, fs, path::PathBuf};

fn test_image(w: usize, h: usize) -> Rgba {
    let mut d = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            d.push(((x * 7 + y * 3) % 256) as u8);
            d.push(((x * 5 + y * 11) % 256) as u8);
            d.push(((x + y * 2) % 256) as u8);
            d.push(((x ^ y) % 256) as u8);
        }
    }
    Rgba::new(w, h, d)
}

fn main() {
    let out = PathBuf::from(env::args().nth(1).expect("usage: dump_frames <outdir>"));
    fs::create_dir_all(&out).unwrap();
    let orients = [
        ("portrait", Orientation::Portrait),
        ("landscape", Orientation::Landscape),
        ("rportrait", Orientation::ReversePortrait),
        ("rlandscape", Orientation::ReverseLandscape),
    ];
    let full = test_image(480, 480);
    let part = test_image(100, 60);
    for (name, o) in orients {
        let mut b = Vec::new();
        for (msg, _) in full_frame_sequence(&full, o) {
            b.extend(msg);
        }
        fs::write(out.join(format!("full_{name}.bin")), b).unwrap();

        let mut b = Vec::new();
        for (msg, _) in update_sequence(&part, 30, 50, 5, o) {
            b.extend(msg);
        }
        fs::write(out.join(format!("update_{name}.bin")), b).unwrap();
    }
    let mut b = Vec::new();
    for lvl in [0u8, 1, 25, 50, 99, 100] {
        b.extend(set_brightness(lvl));
    }
    b.extend(set_options());
    fs::write(out.join("misc.bin"), b).unwrap();
}
