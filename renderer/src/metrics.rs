// SPDX-License-Identifier: GPL-3.0-or-later
//! Metric paths ("cpu.usage", "gpu0.temp", ...) resolved against a sensor snapshot,
//! and `{path:format}` text templates.
//!
//! Paths: cpu.usage cpu.temp cpu.freq cpu.load1 cpu.coreN | mem.used mem.used_gb mem.total_gb
//! mem.avail_gb swap.used | disk.<mount>.used|total_gb|used_gb | net.down net.up net.<iface>.down|up |
//! gpuN.usage|temp|vram|vram_used_gb|vram_total_gb|power|fan | fanN
//! Formats: `.N` (N decimals), `rate` (bytes/s -> "1.2 MB/s"), `bytes` (-> "3.4 GB").

use sensors::Snapshot;

const GIB: f64 = 1_073_741_824.0;

pub fn resolve(s: &Snapshot, path: &str) -> Option<f64> {
    let p = path.trim();
    if let Some(rest) = p.strip_prefix("cpu.") {
        return match rest {
            "usage" => Some(s.cpu.usage_percent as f64),
            "temp" => s.cpu.temp_c.map(f64::from),
            "freq" => s.cpu.freq_mhz.map(f64::from),
            "load1" => s.cpu.load_avg.map(|l| l[0] as f64),
            r => {
                let n: usize = r.strip_prefix("core")?.parse().ok()?;
                s.cpu.per_core_percent.get(n).map(|v| *v as f64)
            }
        };
    }
    if let Some(rest) = p.strip_prefix("mem.") {
        let m = &s.mem;
        return match rest {
            "used" => Some(m.used_percent as f64),
            "used_gb" => Some(m.used_bytes as f64 / GIB),
            "total_gb" => Some(m.total_bytes as f64 / GIB),
            "avail_gb" => Some(m.available_bytes as f64 / GIB),
            _ => None,
        };
    }
    if p == "swap.used" {
        let m = &s.mem;
        return if m.swap_total_bytes == 0 {
            Some(0.0)
        } else {
            Some(100.0 * m.swap_used_bytes as f64 / m.swap_total_bytes as f64)
        };
    }
    if let Some(rest) = p.strip_prefix("disk.") {
        // mount may contain dots: the field is everything after the last '.'
        let (mount, field) = rest.rsplit_once('.')?;
        let d = s.disks.iter().find(|d| d.mount == mount)?;
        return match field {
            "used" => Some(d.used_percent as f64),
            "total_gb" => Some(d.total_bytes as f64 / GIB),
            "used_gb" => Some(d.used_bytes as f64 / GIB),
            _ => None,
        };
    }
    if let Some(rest) = p.strip_prefix("net.") {
        return match rest {
            "down" => Some(s.net.iter().map(|n| n.rx_bytes_per_s).sum()),
            "up" => Some(s.net.iter().map(|n| n.tx_bytes_per_s).sum()),
            r => {
                let (iface, field) = r.rsplit_once('.')?;
                let n = s.net.iter().find(|n| n.name == iface)?;
                match field {
                    "down" => Some(n.rx_bytes_per_s),
                    "up" => Some(n.tx_bytes_per_s),
                    _ => None,
                }
            }
        };
    }
    if let Some(rest) = p.strip_prefix("gpu") {
        let (idx, field) = rest.split_once('.')?;
        let g = s.gpus.get(idx.parse::<usize>().ok()?)?;
        return match field {
            "usage" => g.usage_percent.map(f64::from),
            "temp" => g.temp_c.map(f64::from),
            "power" => g.power_w.map(f64::from),
            "fan" => g.fan_percent.map(f64::from).or(g.fan_rpm.map(f64::from)),
            "vram" => match (g.vram_used_bytes, g.vram_total_bytes) {
                (Some(u), Some(t)) if t > 0 => Some(100.0 * u as f64 / t as f64),
                _ => None,
            },
            "vram_used_gb" => g.vram_used_bytes.map(|v| v as f64 / GIB),
            "vram_total_gb" => g.vram_total_bytes.map(|v| v as f64 / GIB),
            _ => None,
        };
    }
    if let Some(n) = p.strip_prefix("fan") {
        return s.fans.get(n.parse::<usize>().ok()?).map(|f| f.rpm as f64);
    }
    None
}

pub fn human_rate(bytes_per_s: f64) -> String {
    human(bytes_per_s, "/s")
}

pub fn human_bytes(bytes: f64) -> String {
    human(bytes, "")
}

fn human(v: f64, suffix: &str) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut x = v.max(0.0);
    let mut i = 0;
    while x >= 1024.0 && i < UNITS.len() - 1 {
        x /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{:.0} {}{}", x, UNITS[i], suffix)
    } else {
        format!("{:.1} {}{}", x, UNITS[i], suffix)
    }
}

/// Replace every `{path}` / `{path:fmt}` in `tpl`. Unknown or unavailable metrics render as "--".
pub fn render_template(tpl: &str, s: &Snapshot) -> String {
    let mut out = String::new();
    let mut rest = tpl;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let (path, fmt) = match after[..end].split_once(':') {
            Some((p, f)) => (p, f),
            None => (&after[..end], ""),
        };
        match resolve(s, path) {
            None => out.push_str("--"),
            Some(v) => match fmt {
                "rate" => out.push_str(&human_rate(v)),
                "bytes" => out.push_str(&human_bytes(v)),
                f if f.starts_with('.') => {
                    let n: usize = f[1..].parse().unwrap_or(0);
                    out.push_str(&format!("{:.*}", n, v));
                }
                _ => out.push_str(&format!("{:.0}", v)),
            },
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
