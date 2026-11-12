// SPDX-License-Identifier: GPL-3.0-or-later
use crate::fs::Fs;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

/// Parse `/proc/stat`: index 0 is the aggregate "cpu" line, then one entry per core.
pub fn parse_stat(text: &str) -> Vec<CpuTimes> {
    let mut out = Vec::new();
    for line in text.lines() {
        if !line.starts_with("cpu") {
            continue;
        }
        let mut it = line.split_whitespace();
        let name = it.next().unwrap_or("");
        if name != "cpu" && !name[3..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let v: Vec<u64> = it.filter_map(|s| s.parse().ok()).collect();
        if v.len() < 4 {
            continue;
        }
        // user nice system idle iowait irq softirq steal ...
        let idle = v[3] + v.get(4).copied().unwrap_or(0);
        // guest/guest_nice (fields 8, 9) are already counted inside user/nice
        let total: u64 = v.iter().take(8).sum();
        out.push(CpuTimes { idle, total });
    }
    out
}

pub fn usage_percent(prev: CpuTimes, cur: CpuTimes) -> f32 {
    let dt = cur.total.saturating_sub(prev.total);
    if dt == 0 {
        return 0.0;
    }
    let di = cur.idle.saturating_sub(prev.idle).min(dt);
    100.0 * (dt - di) as f32 / dt as f32
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct CpuInfo {
    pub usage_percent: f32,
    pub per_core_percent: Vec<f32>,
    pub freq_mhz: Option<f32>,
    pub temp_c: Option<f32>,
    pub load_avg: Option<[f32; 3]>,
}

pub fn parse_loadavg(text: &str) -> Option<[f32; 3]> {
    let mut it = text.split_whitespace();
    Some([it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next()?.parse().ok()?])
}

/// Mean current frequency of all cores in MHz, from cpufreq (kHz) or /proc/cpuinfo.
pub fn read_freq_mhz(fs: &Fs) -> Option<f32> {
    let mut khz = Vec::new();
    for name in fs.list("sys/devices/system/cpu") {
        if name.starts_with("cpu") && name[3..].chars().all(|c| c.is_ascii_digit()) && name.len() > 3 {
            if let Some(v) = fs.read_u64(&format!("sys/devices/system/cpu/{name}/cpufreq/scaling_cur_freq")) {
                khz.push(v as f32);
            }
        }
    }
    if !khz.is_empty() {
        return Some(khz.iter().sum::<f32>() / khz.len() as f32 / 1000.0);
    }
    let info = fs.read("proc/cpuinfo")?;
    let mhz: Vec<f32> = info
        .lines()
        .filter(|l| l.starts_with("cpu MHz"))
        .filter_map(|l| l.split(':').nth(1)?.trim().parse().ok())
        .collect();
    if mhz.is_empty() {
        None
    } else {
        Some(mhz.iter().sum::<f32>() / mhz.len() as f32)
    }
}
