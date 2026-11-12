// SPDX-License-Identifier: GPL-3.0-or-later
use sensors::cpu::{parse_loadavg, parse_stat, usage_percent, CpuTimes};
use sensors::disk::parse_mounts;
use sensors::gpu::{parse_nvidia_smi, read_amd};
use sensors::hwmon::{cpu_temp, read_chips};
use sensors::mem::parse_meminfo;
use sensors::net::{parse_net_dev, rates};
use sensors::{Fs, Sampler};
use std::fs;
use std::path::Path;

fn put(root: &Path, rel: &str, content: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("sensors-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn stat_and_usage() {
    let a = "cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 50 0 50 400 0 0 0 0 0 0\ncpu1 50 0 50 400 0 0 0 0 0 0\nintr 1\n";
    let b = "cpu  200 0 200 900 0 0 0 0 0 0\ncpu0 100 0 100 450 0 0 0 0 0 0\ncpu1 100 0 100 450 0 0 0 0 0 0\n";
    let (pa, pb) = (parse_stat(a), parse_stat(b));
    assert_eq!(pa.len(), 3);
    // delta: total +300 (user +100, system +100, idle +100) -> 200 busy of 300 = 66.67%
    assert!((usage_percent(pa[0], pb[0]) - 200.0 / 3.0).abs() < 1e-3);
    assert_eq!(usage_percent(CpuTimes { idle: 5, total: 5 }, CpuTimes { idle: 5, total: 5 }), 0.0);
}

#[test]
fn loadavg() {
    assert_eq!(parse_loadavg("0.50 1.00 1.50 2/300 999\n"), Some([0.5, 1.0, 1.5]));
}

#[test]
fn meminfo_used_is_total_minus_available() {
    let t = "MemTotal:       16000000 kB\nMemFree: 1000000 kB\nMemAvailable:    4000000 kB\nSwapTotal: 2000000 kB\nSwapFree: 500000 kB\n";
    let m = parse_meminfo(t).unwrap();
    assert_eq!(m.total_bytes, 16_000_000 * 1024);
    assert_eq!(m.used_bytes, 12_000_000 * 1024);
    assert!((m.used_percent - 75.0).abs() < 1e-4);
    assert_eq!(m.swap_used_bytes, 1_500_000 * 1024);
}

#[test]
fn net_rates_and_loopback() {
    let a = "Inter-|   Receive |  Transmit\n face |bytes packets errs drop fifo frame compressed multicast|bytes packets errs drop fifo colls carrier compressed\n    lo: 100 1 0 0 0 0 0 0 100 1 0 0 0 0 0 0\n  eth0: 1000 1 0 0 0 0 0 0 2000 1 0 0 0 0 0 0\n";
    let b = a.replace("eth0: 1000", "eth0: 3000").replace("0 0 2000 1", "0 0 6000 1");
    let (pa, pb) = (parse_net_dev(a), parse_net_dev(&b));
    assert!(!pa.contains_key("lo"));
    let r = rates(&pa, &pb, 2.0);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].rx_bytes_per_s, 1000.0);
    assert_eq!(r[0].tx_bytes_per_s, 2000.0);
}

#[test]
fn mounts_filter_real_filesystems() {
    let t = "proc /proc proc rw 0 0\n/dev/nvme0n1p2 / ext4 rw 0 0\n/dev/nvme0n1p2 /var/lib/x ext4 rw 0 0\n/dev/sda1 /mnt/my\\040disk ntfs3 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n";
    let m = parse_mounts(t);
    assert_eq!(m, vec![("/".to_string(), "ext4".to_string()), ("/mnt/my disk".to_string(), "ntfs3".to_string())]);
}

#[test]
fn hwmon_cpu_temp_fans_and_amd_gpu() {
    let r = tmp("hw");
    put(&r, "sys/class/hwmon/hwmon0/name", "k10temp\n");
    put(&r, "sys/class/hwmon/hwmon0/temp1_input", "61250\n");
    put(&r, "sys/class/hwmon/hwmon0/temp1_label", "Tctl\n");
    put(&r, "sys/class/hwmon/hwmon1/name", "nct6798\n");
    put(&r, "sys/class/hwmon/hwmon1/fan2_input", "1234\n");
    put(&r, "sys/class/hwmon/hwmon1/fan2_label", "CPU Fan\n");

    let fsx = Fs::at(&r);
    let chips = read_chips(&fsx);
    assert_eq!(chips.len(), 2);
    assert_eq!(cpu_temp(&chips), Some(61.25));
    assert_eq!(chips[1].fans[0].rpm, 1234);

    let d = "sys/class/drm/card0/device";
    put(&r, &format!("{d}/vendor"), "0x1002\n");
    put(&r, &format!("{d}/gpu_busy_percent"), "42\n");
    put(&r, &format!("{d}/mem_info_vram_used"), "1073741824\n");
    put(&r, &format!("{d}/mem_info_vram_total"), "8589934592\n");
    put(&r, &format!("{d}/hwmon/hwmon5/temp1_input"), "55000\n");
    put(&r, &format!("{d}/hwmon/hwmon5/power1_average"), "75000000\n");
    put(&r, &format!("{d}/hwmon/hwmon5/pwm1"), "128\n");
    // a connector entry that must be ignored
    put(&r, "sys/class/drm/card0-DP-1/device/vendor", "0x1002\n");
    let g = read_amd(&fsx);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].usage_percent, Some(42.0));
    assert_eq!(g[0].temp_c, Some(55.0));
    assert_eq!(g[0].power_w, Some(75.0));
    assert_eq!(g[0].vram_total_bytes, Some(8 * 1024 * 1024 * 1024));
    assert!((g[0].fan_percent.unwrap() - 50.2).abs() < 0.1);
    let _ = fs::remove_dir_all(&r);
}

#[test]
fn nvidia_smi_parsing() {
    let t = "NVIDIA GeForce RTX 4070, 33, 48, 1024, 12282, 35.50, [N/A]\n";
    let g = parse_nvidia_smi(t);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].name.as_deref(), Some("NVIDIA GeForce RTX 4070"));
    assert_eq!(g[0].usage_percent, Some(33.0));
    assert_eq!(g[0].vram_used_bytes, Some(1024 * 1_048_576));
    assert_eq!(g[0].power_w, Some(35.5));
    assert_eq!(g[0].fan_percent, None);
}

#[test]
fn sampler_on_fake_root_reports_rates() {
    let r = tmp("sampler");
    let stat = |busy: u64, idle: u64| format!("cpu  {busy} 0 0 {idle} 0 0 0 0 0 0\ncpu0 {busy} 0 0 {idle} 0 0 0 0 0 0\n");
    put(&r, "proc/stat", &stat(100, 900));
    put(&r, "proc/meminfo", "MemTotal: 1000 kB\nMemAvailable: 500 kB\n");
    let mut s = Sampler::with_fs(Fs::at(&r), false);
    put(&r, "proc/stat", &stat(300, 1700)); // +200 busy of +1000 -> 20%
    let snap = s.sample_with_dt(1.0);
    assert!((snap.cpu.usage_percent - 20.0).abs() < 1e-3);
    assert_eq!(snap.cpu.per_core_percent.len(), 1);
    assert!((snap.mem.used_percent - 50.0).abs() < 1e-3);
    let _ = fs::remove_dir_all(&r);
}
