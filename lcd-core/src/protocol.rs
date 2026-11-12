// SPDX-License-Identifier: GPL-3.0-or-later
//
// Wire protocol for Turing "Rev C" screens (2.1" 480x480 variant used by the
// ID-Cooling SL360 XE). Byte values are derived from the upstream project
// turing-smart-screen-python (GPL-3.0-or-later), lcd_comm_rev_c.py.

/// Every command is zero-padded up to a multiple of this many bytes.
pub const BLOCK: usize = 250;
/// Image data is split in chunks of BLOCK-1 bytes, separated by a 0x00 byte.
pub const CHUNK: usize = 249;

pub const HELLO: &[u8] = &[0x01, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xc5, 0xd3];
pub const OPTIONS: &[u8] = &[0x7d, 0xef, 0x69, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x2d];
pub const RESTART: &[u8] = &[0x84, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
pub const TURN_OFF: &[u8] = &[0x83, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
pub const TURN_ON: &[u8] = &[0x83, 0xef, 0x69, 0x00, 0x00, 0x00, 0x00];
pub const SET_BRIGHTNESS: &[u8] = &[0x7b, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
pub const STOP_VIDEO: &[u8] = &[0x79, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
pub const STOP_MEDIA: &[u8] = &[0x96, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
pub const QUERY_STATUS: &[u8] = &[0xcf, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
pub const START_DISPLAY_BITMAP: &[u8] = &[0x2c];
pub const PRE_UPDATE_BITMAP: &[u8] = &[0x86, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
pub const UPDATE_BITMAP: &[u8] = &[0xcc, 0xef, 0x69, 0x00];
/// 0x0E10 = 3600 = 480*480/64
pub const DISPLAY_BITMAP_2INCH: &[u8] = &[0xc8, 0xef, 0x69, 0x00, 0x0e, 0x10];

pub const PAD_NULL: u8 = 0x00;
pub const PAD_START_DISPLAY_BITMAP: u8 = 0x2c;

/// Length of the answer to HELLO.
pub const HELLO_REPLY_LEN: usize = 23;
/// Length of every other status answer.
pub const STATUS_REPLY_LEN: usize = 1024;

/// Pad `msg` with `pad` up to a multiple of 250 bytes (no-op if already a multiple).
pub fn pad_message(mut msg: Vec<u8>, pad: u8) -> Vec<u8> {
    if msg.len() % BLOCK != 0 {
        let target = msg.len().div_ceil(BLOCK) * BLOCK;
        msg.resize(target, pad);
    }
    msg
}

/// Build a padded command: `cmd` followed by `payload`, padded with `pad`.
pub fn build_command(cmd: &[u8], payload: &[u8], pad: u8) -> Vec<u8> {
    let mut m = Vec::with_capacity(cmd.len() + payload.len());
    m.extend_from_slice(cmd);
    m.extend_from_slice(payload);
    pad_message(m, pad)
}

/// Split `data` in 249-byte chunks joined by a 0x00 separator byte.
pub fn chunk_join(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / CHUNK + 1);
    for (i, c) in data.chunks(CHUNK).enumerate() {
        if i > 0 {
            out.push(0x00);
        }
        out.extend_from_slice(c);
    }
    out
}

/// Brightness percent (0-100) to the device's 0-255 scale (truncating, as upstream).
pub fn brightness_byte(percent: u8) -> u8 {
    let p = percent.min(100) as u32;
    ((p * 255) / 100) as u8
}

pub fn set_brightness(percent: u8) -> Vec<u8> {
    build_command(SET_BRIGHTNESS, &[brightness_byte(percent)], PAD_NULL)
}

/// SET_OPTIONS payload used by upstream: start mode default, null, no flip, sleep off.
pub fn set_options() -> Vec<u8> {
    build_command(OPTIONS, &[0x00, 0x00, 0x00, 0x00], PAD_NULL)
}
