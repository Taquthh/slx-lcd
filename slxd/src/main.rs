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
use lcd_core::{Lcd, Orientation};
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
        fps: 1.0,
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
    if !(a.fps > 0.0 && a.fps <= 30.0) {
        return Err("--fps must be in (0, 30]".into());
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
    let mut backoff = 1u64;

    while !STOP.load(Ordering::SeqCst) {
        let t0 = Instant::now();

        if lcd.is_none() {
            match connect(&a) {
                Ok(l) => {
                    lcd = Some(l);
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
                    if let Err(e) = l.display_full(&frame) {
                        eprintln!("send failed: {e}; reconnecting");
                        lcd = None;
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
