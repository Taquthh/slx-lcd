// SPDX-License-Identifier: GPL-3.0-or-later
use serde::Serialize;
use std::ffi::CString;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct DiskUsage {
    pub mount: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub used_percent: f32,
}

const REAL_FS: [&str; 11] =
    ["ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs", "vfat", "exfat", "ntfs", "ntfs3", "zfs"];

/// Mount points of real (block-backed) filesystems from `/proc/mounts` text: (mount, fstype).
pub fn parse_mounts(text: &str) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 || !REAL_FS.contains(&f[2]) {
            continue;
        }
        // /proc/mounts escapes spaces as \040
        let mount = f[1].replace("\\040", " ");
        if seen.insert(f[0].to_string()) {
            // first mount of a device wins (avoids btrfs subvolume / bind duplicates)
            out.push((mount, f[2].to_string()));
        }
    }
    out
}

pub fn statvfs_usage(mount: &str, fs_type: &str) -> Option<DiskUsage> {
    let c = CString::new(mount).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut s) } != 0 {
        return None;
    }
    let bs = s.f_frsize as u64;
    let total = s.f_blocks as u64 * bs;
    let free = s.f_bfree as u64 * bs;
    let avail = s.f_bavail as u64 * bs;
    if total == 0 {
        return None;
    }
    // same convention as `df`: used = total - free, percent against used+avail
    let used = total.saturating_sub(free);
    let denom = used + avail;
    Some(DiskUsage {
        mount: mount.to_string(),
        fs_type: fs_type.to_string(),
        total_bytes: total,
        used_bytes: used,
        used_percent: if denom == 0 { 0.0 } else { 100.0 * used as f32 / denom as f32 },
    })
}
