// SPDX-License-Identifier: GPL-3.0-or-later
use renderer::metrics::{render_template, resolve};
use renderer::{Color, Renderer, Theme};
use sensors::{DiskUsage, GpuInfo, NetIface, Snapshot};
use std::path::Path;

fn snap() -> Snapshot {
    let mut s = Snapshot::default();
    s.cpu.usage_percent = 42.4;
    s.cpu.temp_c = Some(61.0);
    s.cpu.per_core_percent = vec![10.0, 90.0];
    s.mem.total_bytes = 16 * 1_073_741_824;
    s.mem.used_bytes = 8 * 1_073_741_824;
    s.mem.used_percent = 50.0;
    s.disks.push(DiskUsage {
        mount: "/mnt/data.v2".into(),
        fs_type: "ext4".into(),
        total_bytes: 1_073_741_824 * 100,
        used_bytes: 1_073_741_824 * 25,
        used_percent: 25.0,
    });
    s.net.push(NetIface { name: "eth0".into(), rx_bytes_per_s: 1_572_864.0, tx_bytes_per_s: 512.0, ..Default::default() });
    s.gpus.push(GpuInfo { vendor: "AMD".into(), usage_percent: Some(80.0), ..Default::default() });
    s
}

#[test]
fn color_parsing() {
    assert_eq!(Color::parse("#ff8000"), Some(Color(255, 128, 0, 255)));
    assert_eq!(Color::parse("#ff800040"), Some(Color(255, 128, 0, 64)));
    assert_eq!(Color::parse("ff8000"), None);
    assert_eq!(Color::parse("#12345"), None);
}

#[test]
fn metric_paths() {
    let s = snap();
    assert_eq!(resolve(&s, "cpu.usage"), Some(42.4f32 as f64));
    assert_eq!(resolve(&s, "cpu.core1"), Some(90.0));
    assert_eq!(resolve(&s, "cpu.core9"), None);
    assert_eq!(resolve(&s, "mem.total_gb"), Some(16.0));
    // mount with a dot in its name
    assert_eq!(resolve(&s, "disk./mnt/data.v2.used"), Some(25.0));
    assert_eq!(resolve(&s, "net.eth0.down"), Some(1_572_864.0));
    assert_eq!(resolve(&s, "gpu0.usage"), Some(80.0));
    assert_eq!(resolve(&s, "gpu0.temp"), None);
    assert_eq!(resolve(&s, "gpu3.usage"), None);
    assert_eq!(resolve(&s, "nonsense"), None);
}

#[test]
fn templates() {
    let s = snap();
    assert_eq!(render_template("CPU {cpu.usage}%", &s), "CPU 42%");
    assert_eq!(render_template("{cpu.usage:.1}", &s), "42.4");
    assert_eq!(render_template("{net.down:rate} {net.up:rate}", &s), "1.5 MB/s 512 B/s");
    assert_eq!(render_template("{gpu0.temp}°C", &s), "--°C");
    assert_eq!(render_template("open { brace", &s), "open { brace");
}

#[test]
fn bundled_theme_loads_and_renders_opaque_480() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../themes/aqua.json");
    let r = Renderer::from_file(&path).expect("aqua theme");
    let f = r.render_at(&snap(), 1_700_000_000).unwrap();
    assert_eq!((f.width, f.height), (480, 480));
    assert!(f.data.chunks_exact(4).all(|p| p[3] == 255), "frame must be fully opaque");
    // deterministic for the same input
    assert_eq!(f, r.render_at(&snap(), 1_700_000_000).unwrap());
}

#[test]
fn bar_fill_tracks_value() {
    let theme = Theme::from_json(
        r##"{"name":"t","width":100,"height":20,"background":{"color":"#000000"},
        "widgets":[{"type":"bar","x":0,"y":0,"w":100,"h":20,"value":"cpu.usage","fg":"#ffffff","bg":"#000000"}]}"##,
    )
    .unwrap();
    let r = Renderer::new(theme, Path::new(".")).unwrap();
    let mut s = Snapshot::default();
    s.cpu.usage_percent = 50.0;
    let f = r.render_at(&s, 0).unwrap();
    let px = |x: usize| f.data[(10 * 100 + x) * 4];
    assert_eq!(px(25), 255, "filled half is white");
    assert_eq!(px(75), 0, "empty half is black");
}

#[test]
fn invalid_color_is_a_clear_error() {
    let err = Theme::from_json(r##"{"name":"t","widgets":[{"type":"rect","x":0,"y":0,"w":1,"h":1,"color":"red"}]}"##)
        .unwrap_err()
        .to_string();
    assert!(err.contains("invalid color"), "{err}");
}
