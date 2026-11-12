// SPDX-License-Identifier: GPL-3.0-or-later
use crate::fs::Fs;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Temp {
    pub label: String,
    pub celsius: f32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Fan {
    pub chip: String,
    pub label: String,
    pub rpm: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Chip {
    pub name: String,
    /// path relative to the fs root, e.g. `sys/class/hwmon/hwmon3`
    pub dir: String,
    pub temps: Vec<Temp>,
    pub fans: Vec<Fan>,
}

pub fn read_chips(fs: &Fs) -> Vec<Chip> {
    let mut chips = Vec::new();
    for h in fs.list("sys/class/hwmon") {
        if !h.starts_with("hwmon") {
            continue;
        }
        let dir = format!("sys/class/hwmon/{h}");
        let name = fs.read_trim(&format!("{dir}/name")).unwrap_or_else(|| h.clone());
        let mut temps = Vec::new();
        let mut fans = Vec::new();
        for f in fs.list(&dir) {
            if let Some(idx) = f.strip_prefix("temp").and_then(|r| r.strip_suffix("_input")) {
                if let Some(milli) = fs.read_i64(&format!("{dir}/{f}")) {
                    let label = fs.read_trim(&format!("{dir}/temp{idx}_label")).unwrap_or_else(|| format!("temp{idx}"));
                    temps.push(Temp { label, celsius: milli as f32 / 1000.0 });
                }
            } else if let Some(idx) = f.strip_prefix("fan").and_then(|r| r.strip_suffix("_input")) {
                if let Some(rpm) = fs.read_u64(&format!("{dir}/{f}")) {
                    let label = fs.read_trim(&format!("{dir}/fan{idx}_label")).unwrap_or_else(|| format!("fan{idx}"));
                    fans.push(Fan { chip: name.clone(), label, rpm: rpm as u32 });
                }
            }
        }
        chips.push(Chip { name, dir, temps, fans });
    }
    chips
}

/// Best-effort CPU temperature: prefers package/die sensors of known CPU drivers.
pub fn cpu_temp(chips: &[Chip]) -> Option<f32> {
    const DRIVERS: [&str; 4] = ["k10temp", "zenpower", "coretemp", "cpu_thermal"];
    const PREFERRED: [&str; 3] = ["Tctl", "Package id 0", "Tdie"];
    for d in DRIVERS {
        if let Some(c) = chips.iter().find(|c| c.name == d) {
            for p in PREFERRED {
                if let Some(t) = c.temps.iter().find(|t| t.label == p) {
                    return Some(t.celsius);
                }
            }
            if let Some(t) = c.temps.first() {
                return Some(t.celsius);
            }
        }
    }
    None
}

pub fn all_fans(chips: &[Chip]) -> Vec<Fan> {
    chips.iter().flat_map(|c| c.fans.clone()).collect()
}
