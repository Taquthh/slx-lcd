// SPDX-License-Identifier: GPL-3.0-or-later
use crate::fs::Fs;
use serde::Serialize;
use std::process::Command;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct GpuInfo {
    pub vendor: String,
    pub name: Option<String>,
    pub usage_percent: Option<f32>,
    pub temp_c: Option<f32>,
    pub vram_used_bytes: Option<u64>,
    pub vram_total_bytes: Option<u64>,
    pub power_w: Option<f32>,
    pub fan_rpm: Option<u32>,
    pub fan_percent: Option<f32>,
}

/// AMD GPUs through the amdgpu driver's sysfs files (no extra library needed).
pub fn read_amd(fs: &Fs) -> Vec<GpuInfo> {
    let mut out = Vec::new();
    for card in fs.list("sys/class/drm") {
        // only "cardN", not connectors such as "card0-DP-1"
        if !card.starts_with("card") || card[4..].is_empty() || !card[4..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let dev = format!("sys/class/drm/{card}/device");
        if fs.read_trim(&format!("{dev}/vendor")).as_deref() != Some("0x1002") {
            continue;
        }
        let mut g = GpuInfo { vendor: "AMD".into(), ..Default::default() };
        g.usage_percent = fs.read_u64(&format!("{dev}/gpu_busy_percent")).map(|v| v as f32);
        g.vram_used_bytes = fs.read_u64(&format!("{dev}/mem_info_vram_used"));
        g.vram_total_bytes = fs.read_u64(&format!("{dev}/mem_info_vram_total"));
        g.name = fs.read_trim(&format!("{dev}/product_name"));
        for h in fs.list(&format!("{dev}/hwmon")) {
            let hw = format!("{dev}/hwmon/{h}");
            g.temp_c = g.temp_c.or(fs.read_i64(&format!("{hw}/temp1_input")).map(|v| v as f32 / 1000.0));
            // amdgpu reports power in microwatts: prefer average, fall back to input
            g.power_w = g.power_w.or(fs
                .read_u64(&format!("{hw}/power1_average"))
                .or_else(|| fs.read_u64(&format!("{hw}/power1_input")))
                .map(|v| v as f32 / 1_000_000.0));
            g.fan_rpm = g.fan_rpm.or(fs.read_u64(&format!("{hw}/fan1_input")).map(|v| v as u32));
            if let (Some(pwm), Some(max)) = (
                fs.read_u64(&format!("{hw}/pwm1")),
                fs.read_u64(&format!("{hw}/pwm1_max")).or(Some(255)),
            ) {
                if max > 0 {
                    g.fan_percent = g.fan_percent.or(Some(100.0 * pwm as f32 / max as f32));
                }
            }
        }
        out.push(g);
    }
    out
}

/// Parse `nvidia-smi --query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total,power.draw,fan.speed
/// --format=csv,noheader,nounits` output (one GPU per line). "[N/A]" fields become `None`.
pub fn parse_nvidia_smi(text: &str) -> Vec<GpuInfo> {
    let num = |s: &str| s.trim().parse::<f32>().ok();
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            if f.len() < 7 {
                return None;
            }
            Some(GpuInfo {
                vendor: "NVIDIA".into(),
                name: Some(f[0].trim().to_string()),
                usage_percent: num(f[1]),
                temp_c: num(f[2]),
                // memory is reported in MiB
                vram_used_bytes: num(f[3]).map(|v| (v as f64 * 1_048_576.0) as u64),
                vram_total_bytes: num(f[4]).map(|v| (v as f64 * 1_048_576.0) as u64),
                power_w: num(f[5]),
                fan_rpm: None,
                fan_percent: num(f[6]),
            })
        })
        .collect()
}

pub fn read_nvidia() -> Vec<GpuInfo> {
    let out = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,utilization.gpu,temperature.gpu,memory.used,memory.total,power.draw,fan.speed",
            "--format=csv,noheader,nounits",
        ])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_nvidia_smi(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}
