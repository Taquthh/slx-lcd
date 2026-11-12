// SPDX-License-Identifier: GPL-3.0-or-later
use lcd_core::discover::{find_awake, is_sleeping, PortInfo};
use lcd_core::encode::full_frame_sequence;
use lcd_core::protocol::*;
use lcd_core::{Orientation, Rgba};
use std::path::PathBuf;

#[test]
fn padding_is_multiple_of_250() {
    assert_eq!(pad_message(vec![1; 1], 0).len(), 250);
    assert_eq!(pad_message(vec![1; 250], 0).len(), 250);
    assert_eq!(pad_message(vec![1; 251], 0).len(), 500);
}

#[test]
fn chunk_join_inserts_separator() {
    let j = chunk_join(&vec![7u8; 250]);
    assert_eq!(j.len(), 251);
    assert_eq!(j[249], 0x00);
}

#[test]
fn brightness_matches_upstream_truncation() {
    assert_eq!(brightness_byte(0), 0);
    assert_eq!(brightness_byte(25), 63);
    assert_eq!(brightness_byte(100), 255);
}

#[test]
fn rotations_roundtrip() {
    let img = Rgba::new(3, 2, (0..24).collect());
    assert_eq!(img.rotate90().rotate270(), img);
    assert_eq!(img.rotate180().rotate180(), img);
    assert_eq!(img.rotate90().rotate90(), img.rotate180());
}

#[test]
fn full_frame_has_five_steps_and_size() {
    let img = Rgba::solid(480, 480, [1, 2, 3, 255]);
    let seq = full_frame_sequence(&img, Orientation::Landscape);
    assert_eq!(seq.len(), 5);
    assert_eq!(seq[3].0.len(), 925_500);
    assert!(seq.iter().all(|(m, _)| m.len() % 250 == 0));
}

#[test]
fn port_classification() {
    let sleeping = PortInfo { device: PathBuf::from("/dev/ttyACM0"), vid: Some(0x1a86), pid: Some(0xca21), serial: None };
    let awake = PortInfo { device: PathBuf::from("/dev/ttyACM1"), vid: Some(0x0525), pid: Some(0xa4a7), serial: None };
    assert!(is_sleeping(&sleeping));
    assert_eq!(find_awake(&[sleeping, awake.clone()]), Some(&awake));
}
