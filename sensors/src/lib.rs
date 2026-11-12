// SPDX-License-Identifier: GPL-3.0-or-later
//! Linux system sensors for the LCD app, read straight from `/proc` and `/sys`.
//!
//! Create a [`Sampler`] once and call [`Sampler::sample`] periodically (e.g. every second):
//! CPU usage and network rates are computed from the difference with the previous sample.

pub mod cpu;
pub mod disk;
pub mod fs;
pub mod gpu;
pub mod hwmon;
pub mod mem;
pub mod net;

use std::collections::HashMap;
use std::time::Instant;

use serde::Serialize;

pub use cpu::CpuInfo;
pub use disk::DiskUsage;
pub use fs::Fs;
pub use gpu::GpuInfo;
pub use hwmon::{Chip, Fan, Temp};
pub use mem::MemInfo;
pub use net::NetIface;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Snapshot {
    pub cpu: CpuInfo,
    pub mem: MemInfo,
    pub disks: Vec<DiskUsage>,
    pub net: Vec<NetIface>,
    pub fans: Vec<Fan>,
    pub gpus: Vec<GpuInfo>,
    pub hwmon: Vec<Chip>,
}

pub struct Sampler {
    fs: Fs,
    use_nvidia_smi: bool,
    prev_cpu: Vec<cpu::CpuTimes>,
    prev_net: HashMap<String, net::NetCounters>,
    prev_at: Instant,
}

impl Sampler {
    pub fn new() -> Self {
        Self::with_fs(Fs::system(), true)
    }

    /// `use_nvidia_smi = false` skips spawning `nvidia-smi` (used by tests).
    pub fn with_fs(fs: Fs, use_nvidia_smi: bool) -> Self {
        let prev_cpu = fs.read("proc/stat").map(|t| cpu::parse_stat(&t)).unwrap_or_default();
        let prev_net = fs.read("proc/net/dev").map(|t| net::parse_net_dev(&t)).unwrap_or_default();
        Self { fs, use_nvidia_smi, prev_cpu, prev_net, prev_at: Instant::now() }
    }

    /// Take a snapshot; rates are measured against the previous call (or `new`).
    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let dt = now.duration_since(self.prev_at).as_secs_f64();
        self.prev_at = now;
        self.sample_with_dt(dt)
    }

    /// Same as [`sample`](Self::sample) with an explicit elapsed time in seconds.
    pub fn sample_with_dt(&mut self, dt: f64) -> Snapshot {
        let chips = hwmon::read_chips(&self.fs);

        // CPU
        let cur_cpu = self.fs.read("proc/stat").map(|t| cpu::parse_stat(&t)).unwrap_or_default();
        let mut cpu_info = CpuInfo::default();
        if let (Some(c0), Some(p0)) = (cur_cpu.first(), self.prev_cpu.first()) {
            cpu_info.usage_percent = cpu::usage_percent(*p0, *c0);
        }
        for (i, c) in cur_cpu.iter().enumerate().skip(1) {
            let p = self.prev_cpu.get(i).copied().unwrap_or_default();
            cpu_info.per_core_percent.push(cpu::usage_percent(p, *c));
        }
        cpu_info.freq_mhz = cpu::read_freq_mhz(&self.fs);
        cpu_info.temp_c = hwmon::cpu_temp(&chips);
        cpu_info.load_avg = self.fs.read("proc/loadavg").and_then(|t| cpu::parse_loadavg(&t));
        self.prev_cpu = cur_cpu;

        // Memory
        let mem = self.fs.read("proc/meminfo").and_then(|t| mem::parse_meminfo(&t)).unwrap_or_default();

        // Disks
        let disks = self
            .fs
            .read("proc/mounts")
            .map(|t| disk::parse_mounts(&t))
            .unwrap_or_default()
            .iter()
            .filter_map(|(m, t)| disk::statvfs_usage(&self.fs.path(m).to_string_lossy(), t).map(|mut d| {
                d.mount = m.clone();
                d
            }))
            .collect();

        // Network
        let cur_net = self.fs.read("proc/net/dev").map(|t| net::parse_net_dev(&t)).unwrap_or_default();
        let net = net::rates(&self.prev_net, &cur_net, dt);
        self.prev_net = cur_net;

        // GPUs
        let mut gpus = gpu::read_amd(&self.fs);
        if self.use_nvidia_smi {
            gpus.extend(gpu::read_nvidia());
        }

        Snapshot { cpu: cpu_info, mem, disks, net, fans: hwmon::all_fans(&chips), gpus, hwmon: chips }
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}
