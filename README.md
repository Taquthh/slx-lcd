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

## Aplikasi GUI desktop

GUI desktop Tauri menyediakan pratinjau sensor dan editor tema. Saat aplikasi dibuka, tema terakhir otomatis dimuat dan dikirim ke LCD. Pilih tema atau edit JSON, lalu tekan satu tombol **Muat ke LCD** untuk menerapkannya. Tombol **Hentikan** menghentikan pengiriman frame serta polling sensor. Opsi **Muat tema terakhir saat sistem mulai** menambahkan aplikasi ke autostart sesi pengguna. Aplikasi hanya mengubah tampilan LCD, bukan kecepatan pompa atau kipas cooler. Saat GUI mengambil alih atau dihentikan, service pengguna `slxd.service` dihentikan agar tidak berebut port serial. Di Debian/Ubuntu, pasang dependensi GUI berikut sekali:

```bash
sudo apt install build-essential curl file libssl-dev libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev
```

Pastikan Rust/Cargo sudah terpasang, lalu dari folder utama repo jalankan:

```bash
bash packaging/install-gui.sh
```

Setelah selesai, buka menu aplikasi desktop dan cari **SLX LCD**. Script membangun dan memasang aplikasi serta daemon ke `~/.local/bin`, menyalin tema awal, dan mendaftarkan entri menu desktop tanpa memerlukan instalasi Tauri CLI. Untuk akses perangkat LCD, jalankan `bash packaging/install.sh` satu kali agar aturan udev dipasang; script tersebut meminta sudo untuk langkah itu. Hentikan program lain yang sedang memakai port serial LCD sebelum menekan **Kirim ke layar**.

Untuk pengembangan tanpa instalasi, di Debian/Ubuntu pasang dependensi GUI di atas lalu jalankan `cargo run --release --manifest-path app/src-tauri/Cargo.toml` dari folder repo.
