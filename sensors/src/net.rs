// SPDX-License-Identifier: GPL-3.0-or-later
use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetCounters {
    pub rx: u64,
    pub tx: u64,
}

/// Parse `/proc/net/dev` into (interface -> counters). Loopback is skipped.
pub fn parse_net_dev(text: &str) -> HashMap<String, NetCounters> {
    let mut out = HashMap::new();
    for line in text.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else { continue };
        let name = name.trim();
        if name == "lo" {
            continue;
        }
        let v: Vec<u64> = rest.split_whitespace().filter_map(|s| s.parse().ok()).collect();
        if v.len() >= 9 {
            out.insert(name.to_string(), NetCounters { rx: v[0], tx: v[8] });
        }
    }
    out
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct NetIface {
    pub name: String,
    pub rx_bytes_per_s: f64,
    pub tx_bytes_per_s: f64,
    pub rx_total: u64,
    pub tx_total: u64,
}

/// Per-interface rate between two samples `dt` seconds apart (counter resets give 0).
pub fn rates(
    prev: &HashMap<String, NetCounters>,
    cur: &HashMap<String, NetCounters>,
    dt: f64,
) -> Vec<NetIface> {
    let mut v: Vec<NetIface> = cur
        .iter()
        .map(|(name, c)| {
            let (rx, tx) = match prev.get(name) {
                Some(p) if dt > 0.0 => (
                    c.rx.saturating_sub(p.rx) as f64 / dt,
                    c.tx.saturating_sub(p.tx) as f64 / dt,
                ),
                _ => (0.0, 0.0),
            };
            NetIface { name: name.clone(), rx_bytes_per_s: rx, tx_bytes_per_s: tx, rx_total: c.rx, tx_total: c.tx }
        })
        .collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}
