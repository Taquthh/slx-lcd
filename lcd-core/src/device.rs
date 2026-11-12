// SPDX-License-Identifier: GPL-3.0-or-later
//
// Live connection to the screen: wake-up, HELLO handshake and frame transfer.

use std::io::{Read, Write};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use serialport::{FlowControl, SerialPort};
use thiserror::Error;

use crate::discover::{find_awake, find_sleeping, list_ports};
use crate::encode::{full_frame_sequence, update_sequence};
use crate::image::{Orientation, Rgba};
use crate::protocol::*;

const WAKE_RETRIES: usize = 15;

#[derive(Debug, Error)]
pub enum LcdError {
    #[error("serial error: {0}")]
    Serial(#[from] serialport::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("no screen found (is it plugged in? check udev permissions)")]
    NotFound,
    #[error("screen returned an invalid ID: {0:?}")]
    BadHello(String),
}

pub struct Lcd {
    port: Box<dyn SerialPort>,
    pub orientation: Orientation,
    pub display_id: String,
    pub rom_version: u32,
    count: u32,
}

/// Try to wake a sleeping screen by connecting to it, then return the awake port path.
pub fn wake_and_find() -> Result<String, LcdError> {
    let ports = list_ports();
    if find_awake(&ports).is_none() {
        for sp in find_sleeping(&ports) {
            for _ in 0..WAKE_RETRIES {
                let _ = serialport::new(sp.device.to_string_lossy(), 115_200)
                    .timeout(Duration::from_secs(1))
                    .flow_control(FlowControl::Hardware)
                    .open();
                if find_awake(&list_ports()).is_some() {
                    sleep(Duration::from_secs(1));
                    break;
                }
                sleep(Duration::from_secs(1));
            }
        }
    }
    let ports = list_ports();
    find_awake(&ports)
        .map(|p| p.device.to_string_lossy().to_string())
        .ok_or(LcdError::NotFound)
}

fn printable(bytes: &[u8]) -> String {
    bytes.iter().filter(|b| (0x20..0x7f).contains(*b)).map(|b| *b as char).collect()
}

impl Lcd {
    /// Open `path` (or auto-detect when `None`) and perform the HELLO handshake.
    pub fn connect(path: Option<&Path>) -> Result<Self, LcdError> {
        let dev = match path {
            Some(p) => p.to_string_lossy().to_string(),
            None => wake_and_find()?,
        };
        let port = serialport::new(&dev, 115_200)
            .timeout(Duration::from_secs(1))
            .flow_control(FlowControl::Hardware)
            .open()?;
        let mut lcd = Lcd {
            port,
            orientation: Orientation::Landscape,
            display_id: String::new(),
            rom_version: 87,
            count: 0,
        };
        lcd.hello()?;
        Ok(lcd)
    }

    fn write_msg(&mut self, msg: &[u8]) -> Result<(), LcdError> {
        self.port.write_all(msg)?;
        Ok(())
    }

    fn read_n(&mut self, n: usize) -> Result<Vec<u8>, LcdError> {
        let mut buf = vec![0u8; n];
        let mut got = 0;
        while got < n {
            match self.port.read(&mut buf[got..]) {
                Ok(0) => break,
                Ok(k) => got += k,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(e.into()),
            }
        }
        buf.truncate(got);
        Ok(buf)
    }

    fn hello(&mut self) -> Result<(), LcdError> {
        let _ = self.port.clear(serialport::ClearBuffer::Input);
        let mut last = String::new();
        for _ in 0..10 {
            self.write_msg(&build_command(HELLO, &[], PAD_NULL))?;
            let reply = self.read_n(HELLO_REPLY_LEN)?;
            let _ = self.port.clear(serialport::ClearBuffer::Input);
            last = printable(&reply);
            if last.starts_with("chs_") {
                self.rom_version = last
                    .split('.')
                    .nth(2)
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|v| (80..=100).contains(v))
                    .unwrap_or(87);
                self.display_id = last;
                return Ok(());
            }
            sleep(Duration::from_secs(1));
        }
        Err(LcdError::BadHello(last))
    }

    fn run(&mut self, seq: Vec<(Vec<u8>, usize)>) -> Result<(), LcdError> {
        for (msg, read) in seq {
            self.write_msg(&msg)?;
            if read > 0 {
                self.read_n(read)?;
            }
        }
        Ok(())
    }

    pub fn set_brightness(&mut self, percent: u8) -> Result<(), LcdError> {
        self.write_msg(&set_brightness(percent))
    }

    pub fn set_orientation(&mut self, o: Orientation) -> Result<(), LcdError> {
        self.orientation = o;
        self.write_msg(&set_options())
    }

    pub fn screen_off(&mut self) -> Result<(), LcdError> {
        self.write_msg(&build_command(STOP_VIDEO, &[], PAD_NULL))?;
        self.write_msg(&build_command(STOP_MEDIA, &[], PAD_NULL))?;
        self.read_n(STATUS_REPLY_LEN)?;
        self.write_msg(&build_command(TURN_OFF, &[], PAD_NULL))
    }

    pub fn screen_on(&mut self) -> Result<(), LcdError> {
        self.write_msg(&build_command(STOP_VIDEO, &[], PAD_NULL))?;
        self.write_msg(&build_command(STOP_MEDIA, &[], PAD_NULL))?;
        self.read_n(STATUS_REPLY_LEN)?;
        Ok(())
    }

    pub fn restart(&mut self) -> Result<(), LcdError> {
        self.write_msg(&build_command(RESTART, &[], PAD_NULL))
    }

    /// Send a full 480x480 frame.
    pub fn display_full(&mut self, img: &Rgba) -> Result<(), LcdError> {
        let seq = full_frame_sequence(img, self.orientation);
        self.run(seq)
    }

    /// Send a partial update at (x, y).
    pub fn display_region(&mut self, img: &Rgba, x: usize, y: usize) -> Result<(), LcdError> {
        let seq = update_sequence(img, x, y, self.count, self.orientation);
        self.count = self.count.wrapping_add(1);
        self.run(seq)
    }
}
