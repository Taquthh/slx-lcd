// SPDX-License-Identifier: GPL-3.0-or-later
// Prints one JSON snapshot after ~1 second (so CPU/network rates are meaningful).
use sensors::Sampler;
use std::{thread::sleep, time::Duration};

fn main() {
    let mut s = Sampler::new();
    sleep(Duration::from_secs(1));
    println!("{}", serde_json::to_string_pretty(&s.sample()).unwrap());
}
