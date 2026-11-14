// SPDX-License-Identifier: GPL-3.0-or-later
//! slxd: draw a theme with live sensors on the SL360 XE LCD.
//!
//! slxd [--theme FILE] [--port /dev/ttyACM0] [--fps N] [--brightness 0-100]
//!      [--orientation landscape|portrait|rportrait|rlandscape]
//!      [--preview out.png] [--once] [--list-ports]

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, Instant};

use lcd_core::discover::list_ports;
use lcd_core::{Lcd, Orientation, Rgba};
use renderer::Renderer;
use sensors::Sampler;

static STOP: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}

struct Args {
    theme: PathBuf,
    port: Option<PathBuf>,
    fps: f64,
    brightness: u8,
    orientation: Orientation,
    preview: Option<PathBuf>,
    once: bool,
    list_ports: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        theme: PathBuf::from("themes/aqua.json"),
        port: None,
        fps: 60.0,
        brightness: 60,
        orientation: Orientation::Landscape,
        preview: None,
        once: false,
        list_ports: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut val = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match k.as_str() {
            "--theme" => a.theme = PathBuf::from(val("--theme")?),
            "--port" => a.port = Some(PathBuf::from(val("--port")?)),
            "--fps" => a.fps = val("--fps")?.parse().map_err(|_| "--fps must be a number")?,
            "--brightness" => a.brightness = val("--brightness")?.parse().map_err(|_| "--brightness must be 0-100")?,
            "--orientation" => {
                a.orientation = match val("--orientation")?.as_str() {
                    "landscape" => Orientation::Landscape,
                    "portrait" => Orientation::Portrait,
                    "rportrait" => Orientation::ReversePortrait,
                    "rlandscape" => Orientation::ReverseLandscape,
                    o => return Err(format!("unknown orientation {o:?}")),
                }
            }
            "--preview" => a.preview = Some(PathBuf::from(val("--preview")?)),
            "--once" => a.once = true,
            "--list-ports" => a.list_ports = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    if a.brightness > 100 {
        return Err("--brightness must be 0-100".into());
    }
    if !(a.fps >= 1.0 && a.fps <= 60.0) {
        return Err("--fps must be in [1, 60]".into());
    }
    Ok(a)
}

const USAGE: &str = "usage: slxd [--theme FILE] [--port DEV] [--fps N] [--brightness 0-100] \
[--orientation landscape|portrait|rportrait|rlandscape] [--preview out.png] [--once] [--list-ports]";

fn connect(a: &Args) -> Result<Lcd, lcd_core::LcdError> {
    let mut lcd = Lcd::connect(a.port.as_deref())?;
    eprintln!("connected: {} (ROM {})", lcd.display_id, lcd.rom_version);
    lcd.set_brightness(a.brightness)?;
    lcd.set_orientation(a.orientation)?;
    Ok(lcd)
}

fn changed_bounds(previous: &Rgba, current: &Rgba) -> Option<(usize, usize, usize, usize)> {
    if previous.width != current.width || previous.height != current.height {
        return Some((0, 0, current.width, current.height));
    }

    let (mut min_x, mut min_y) = (current.width, current.height);
    let (mut max_x, mut max_y) = (0, 0);
    let mut changed = false;
    for y in 0..current.height {
        for x in 0..current.width {
            let index = (y * current.width + x) * 4;
            if previous.data[index..index + 4] != current.data[index..index + 4] {
                changed = true;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    changed.then_some((min_x, min_y, max_x - min_x + 1, max_y - min_y + 1))
}

fn crop_region(frame: &Rgba, x: usize, y: usize, width: usize, height: usize) -> Rgba {
    let mut data = Vec::with_capacity(width * height * 4);
    for row in y..y + height {
        let start = (row * frame.width + x) * 4;
        data.extend_from_slice(&frame.data[start..start + width * 4]);
    }
    Rgba::new(width, height, data)
}

fn main() -> ExitCode {
    let a = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("error: {e}");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    if a.list_ports {
        for p in list_ports() {
            println!(
                "{}  vid={} pid={} serial={}",
                p.device.display(),
                p.vid.map(|v| format!("{v:04x}")).unwrap_or_else(|| "-".into()),
                p.pid.map(|v| format!("{v:04x}")).unwrap_or_else(|| "-".into()),
                p.serial.as_deref().unwrap_or("-")
            );
        }
        return ExitCode::SUCCESS;
    }

    let rd = match Renderer::from_file(&a.theme) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if rd.theme.width != 480 || rd.theme.height != 480 {
        eprintln!("error: the SL360 XE needs a 480x480 theme (got {}x{})", rd.theme.width, rd.theme.height);
        return ExitCode::FAILURE;
    }

    let mut sampler = Sampler::new();

    // Preview mode: no screen needed.
    if let Some(out) = &a.preview {
        sleep(Duration::from_millis(1000));
        let frame = match rd.render(&sampler.sample()) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        };
        match renderer::to_png(&frame).map(|png| std::fs::write(out, png)) {
            Some(Ok(())) => {
                eprintln!("wrote {}", out.display());
                return ExitCode::SUCCESS;
            }
            _ => {
                eprintln!("error: cannot write {}", out.display());
                return ExitCode::FAILURE;
            }
        }
    }

    unsafe {
        let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }

    let period = Duration::from_secs_f64(1.0 / a.fps);
    let mut lcd: Option<Lcd> = None;
    let mut last_frame: Option<Rgba> = None;
    let mut backoff = 1u64;

    while !STOP.load(Ordering::SeqCst) {
        let t0 = Instant::now();

        if lcd.is_none() {
            match connect(&a) {
                Ok(l) => {
                    lcd = Some(l);
                    last_frame = None;
                    backoff = 1;
                }
                Err(e) => {
                    eprintln!("screen not ready: {e} (retry in {backoff}s)");
                    for _ in 0..backoff * 10 {
                        if STOP.load(Ordering::SeqCst) {
                            break;
                        }
                        sleep(Duration::from_millis(100));
                    }
                    backoff = (backoff * 2).min(30);
                    continue;
                }
            }
        }

        let snap = sampler.sample();
        match rd.render(&snap) {
            Ok(frame) => {
                if let Some(l) = lcd.as_mut() {
                    let result = match last_frame.as_ref() {
                        None => l.display_full(&frame),
                        Some(previous) => match changed_bounds(previous, &frame) {
                            Some((x, y, width, height)) => {
                                let patch = crop_region(&frame, x, y, width, height);
                                l.display_region(&patch, x, y)
                            }
                            None => Ok(()),
                        },
                    };
                    if let Err(e) = result {
                        eprintln!("send failed: {e}; reconnecting");
                        lcd = None;
                        last_frame = None;
                    } else {
                        last_frame = Some(frame);
                    }
                }
            }
            Err(e) => eprintln!("render failed: {e}"),
        }

        if a.once {
            break;
        }
        if let Some(rest) = period.checked_sub(t0.elapsed()) {
            sleep(rest);
        }
    }

    if let Some(l) = lcd.as_mut() {
        let _ = l.set_brightness(a.brightness);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::{changed_bounds, crop_region};
    use lcd_core::Rgba;

    #[test]
    fn changed_bounds_crops_only_changed_pixels() {
        let previous = Rgba::solid(4, 3, [0, 0, 0, 255]);
        let mut current = previous.clone();
        let index = (2 * current.width + 1) * 4;
        current.data[index..index + 4].copy_from_slice(&[255, 0, 0, 255]);
        assert_eq!(changed_bounds(&previous, &current), Some((1, 2, 1, 1)));
        let patch = crop_region(&current, 1, 2, 1, 1);
        assert_eq!((patch.width, patch.height), (1, 1));
        assert_eq!(patch.data, [255, 0, 0, 255]);
    }

    #[test]
    fn unchanged_frame_has_no_update_region() {
        let frame = Rgba::solid(3, 2, [8, 16, 24, 255]);
        assert_eq!(changed_bounds(&frame, &frame), None);
    }
}
