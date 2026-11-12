// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocol core for the ID-Cooling SL360 XE 2.1" LCD (Turing Rev C compatible).

pub mod device;
pub mod discover;
pub mod encode;
pub mod image;
pub mod protocol;

pub use device::{Lcd, LcdError};
pub use image::{Orientation, Rgba};
