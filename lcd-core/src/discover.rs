// SPDX-License-Identifier: GPL-3.0-or-later
//
// Serial port discovery through sysfs (no libudev dependency).

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortInfo {
    pub device: PathBuf, // e.g. /dev/ttyACM0
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub serial: Option<String>,
}

fn read_trim(p: &Path) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn read_hex(p: &Path) -> Option<u16> {
    read_trim(p).and_then(|s| u16::from_str_radix(&s, 16).ok())
}

/// Walk up from the tty's sysfs device node until the USB device (with idVendor) is found.
fn usb_info(tty_sys: &Path) -> (Option<u16>, Option<u16>, Option<String>) {
    let Ok(mut cur) = fs::canonicalize(tty_sys.join("device")) else {
        return (None, None, None);
    };
    for _ in 0..6 {
        if cur.join("idVendor").exists() {
            return (
                read_hex(&cur.join("idVendor")),
                read_hex(&cur.join("idProduct")),
                read_trim(&cur.join("serial")),
            );
        }
        if !cur.pop() {
            break;
        }
    }
    (None, None, None)
}

/// List tty ports (ttyACM*, ttyUSB*) found under `sys_class_tty`.
pub fn list_ports_in(sys_class_tty: &Path, dev_dir: &Path) -> Vec<PortInfo> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(sys_class_tty) else {
        return out;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !(name.starts_with("ttyACM") || name.starts_with("ttyUSB")) {
            continue;
        }
        let (vid, pid, serial) = usb_info(&e.path());
        out.push(PortInfo { device: dev_dir.join(&name), vid, pid, serial });
    }
    out.sort_by(|a, b| a.device.cmp(&b.device));
    out
}

pub fn list_ports() -> Vec<PortInfo> {
    list_ports_in(Path::new("/sys/class/tty"), Path::new("/dev"))
}

/// Port of a screen in "sleeping" state (needs a wake-up connect).
pub fn is_sleeping(p: &PortInfo) -> bool {
    matches!(p.serial.as_deref(), Some("USB7INCH") | Some("CT21INCH") | Some("CT88INCH"))
        || (p.vid == Some(0x1a86) && p.pid == Some(0xca21))
}

/// Port of a screen in "awake" state, ready for the protocol.
pub fn is_awake(p: &PortInfo) -> bool {
    p.serial.as_deref() == Some("20080411")
        || (p.vid == Some(0x0525) && p.pid == Some(0xa4a7))
        || (p.vid == Some(0x1d6b) && matches!(p.pid, Some(0x0121) | Some(0x0106)))
}

pub fn find_awake(ports: &[PortInfo]) -> Option<&PortInfo> {
    ports.iter().find(|p| is_awake(p))
}

pub fn find_sleeping(ports: &[PortInfo]) -> Vec<&PortInfo> {
    ports.iter().filter(|p| is_sleeping(p)).collect()
}
