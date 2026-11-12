# SLX LCD: ID-Cooling SL360 XE di Linux

Aplikasi pengganti software vendor untuk LCD 2.1" (480x480) pada ID-Cooling SL360 XE.
Protokol layar diturunkan dari [turing-smart-screen-python](https://github.com/mathoudebine/turing-smart-screen-python)
(driver Rev C), jadi proyek ini berlisensi **GPL-3.0-or-later**.

## Isi

| Folder | Fungsi | Status |
|---|---|---|
| `lcd-core` | protokol layar, deteksi port, kirim frame | teruji, byte identik dengan Python upstream |
| `sensors` | CPU, RAM, disk, jaringan, kipas, GPU dari `/proc` dan `/sys` | teruji dengan data palsu |
| `renderer` | tema JSON + sensor → gambar 480x480 | teruji |
| `slxd` | daemon: sensor → render → layar, mode `--preview` | berjalan; kirim ke layar fisik belum diuji |
| `app` | cangkang Tauri + UI pratinjau/editor tema | **belum pernah dikompilasi** |
| `packaging` | udev rule, service systemd pengguna, `install.sh` | sintaks diperiksa saja |

## Pakai tanpa GUI

```bash
cargo build --release -p slxd
./target/release/slxd --list-ports                       # lihat port serial
./target/release/slxd --theme themes/aqua.json --preview /tmp/p.png   # tanpa layar
./target/release/slxd --theme themes/aqua.json --brightness 60         # kirim ke layar
packaging/install.sh && systemctl --user enable --now slxd             # otomatis saat login
```

Hentikan dulu `main.py` dari turing-smart-screen-python: hanya satu program yang boleh memakai port.

## Tema

Lihat `themes/aqua.json`. Widget: `rect`, `text`, `bar`, `gauge`, `clock`, `image`.
Teks memakai `{metrik}` atau `{metrik:.1}`, `{net.down:rate}`. Metrik: `cpu.usage cpu.temp cpu.freq cpu.coreN`,
`mem.used mem.used_gb mem.total_gb`, `disk./.used`, `net.down net.up net.<iface>.down`,
`gpuN.usage gpuN.temp gpuN.vram gpuN.power`, `fanN`. Metrik yang tidak tersedia tampil `--`.

## Cangkang Tauri

Butuh `webkit2gtk-4.1` dan GTK3 (paket dev). Di Debian/Ubuntu:
`sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`

```bash
cd app/src-tauri
cargo install tauri-cli --version "^2"
cargo tauri icon path/to/logo.png    # menghasilkan folder icons/ yang dibutuhkan bundel
cargo tauri dev
```
