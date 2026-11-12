// SPDX-License-Identifier: GPL-3.0-or-later
use std::fs;
use std::path::{Path, PathBuf};

/// Filesystem root abstraction so parsers can be tested against a fake `/proc` and `/sys`.
#[derive(Clone, Debug)]
pub struct Fs {
    root: PathBuf,
}

impl Fs {
    pub fn system() -> Self {
        Self { root: PathBuf::from("/") }
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel.trim_start_matches('/'))
    }

    pub fn read(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.path(rel)).ok()
    }

    pub fn read_trim(&self, rel: &str) -> Option<String> {
        self.read(rel).map(|s| s.trim().to_string())
    }

    pub fn read_u64(&self, rel: &str) -> Option<u64> {
        self.read_trim(rel)?.parse().ok()
    }

    pub fn read_i64(&self, rel: &str) -> Option<i64> {
        self.read_trim(rel)?.parse().ok()
    }

    /// Sorted entry names of a directory (empty if missing).
    pub fn list(&self, rel: &str) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(self.path(rel))
            .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        v.sort();
        v
    }

    pub fn exists(&self, rel: &str) -> bool {
        Path::new(&self.path(rel)).exists()
    }
}
