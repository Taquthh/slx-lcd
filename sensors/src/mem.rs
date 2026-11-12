// SPDX-License-Identifier: GPL-3.0-or-later
use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct MemInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub used_percent: f32,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

fn kb(text: &str, key: &str) -> Option<u64> {
    text.lines()
        .find(|l| l.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()
        .map(|v| v * 1024)
}

/// Parse `/proc/meminfo`. "Used" = total - MemAvailable (same as `free`'s "used").
pub fn parse_meminfo(text: &str) -> Option<MemInfo> {
    let total = kb(text, "MemTotal:")?;
    let avail = kb(text, "MemAvailable:").or_else(|| {
        Some(kb(text, "MemFree:")? + kb(text, "Buffers:").unwrap_or(0) + kb(text, "Cached:").unwrap_or(0))
    })?;
    let used = total.saturating_sub(avail);
    let st = kb(text, "SwapTotal:").unwrap_or(0);
    let sf = kb(text, "SwapFree:").unwrap_or(0);
    Some(MemInfo {
        total_bytes: total,
        used_bytes: used,
        available_bytes: avail,
        used_percent: if total == 0 { 0.0 } else { 100.0 * used as f32 / total as f32 },
        swap_total_bytes: st,
        swap_used_bytes: st.saturating_sub(sf),
    })
}
