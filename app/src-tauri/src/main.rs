// SPDX-License-Identifier: GPL-3.0-or-later
//! Tauri shell: theme editing with live preview, plus start/stop of the `slxd` daemon.
//! The heavy lifting stays in the library crates (sensors, renderer, lcd-core).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;

use base64::Engine;
use serde::{Deserialize, Serialize};
use renderer::{Renderer, Theme};
use sensors::{Sampler, Snapshot};

struct AppState {
    sampler: Mutex<Sampler>,
    daemon: Mutex<Option<Child>>,
    settings: Mutex<AppSettings>,
}

impl Drop for AppState {
    fn drop(&mut self) {
        if let Ok(daemon) = self.daemon.get_mut() {
            if let Some(mut child) = daemon.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct AppSettings {
    last_theme: String,
    brightness: u8,
    orientation: String,
    start_on_boot: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_theme: "aqua".into(),
            brightness: 60,
            orientation: "landscape".into(),
            start_on_boot: false,
        }
    }
}

fn config_home() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn config_dir() -> PathBuf {
    config_home().join("slxd")
}

fn settings_path() -> PathBuf {
    config_dir().join("app-settings.json")
}

fn load_app_settings() -> AppSettings {
    fs::read_to_string(settings_path())
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn sync_autostart(enabled: bool) -> Result<(), String> {
    let path = config_home().join("autostart/slx-lcd.desktop");
    if !enabled {
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let executable = executable
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    let contents = format!(
        "[Desktop Entry]\nType=Application\nName=SLX LCD\nComment=Load the last SLX LCD theme\nExec=\"{executable}\"\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
    );
    let parent = path.parent().ok_or("invalid autostart path")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::write(path, contents).map_err(|e| e.to_string())
}

fn themes_dir() -> PathBuf {
    config_dir().join("themes")
}

/// Theme names are file stems: refuse anything that could escape the themes folder.
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 64 && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[tauri::command]
fn list_themes() -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(themes_dir())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let p = e.path();
                    p.extension()
                        .filter(|extension| *extension == "json")
                        .and_then(|_| p.file_stem())
                        .map(|stem| stem.to_string_lossy().into_owned())
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[tauri::command]
fn read_theme(name: String) -> Result<String, String> {
    if !valid_name(&name) {
        return Err("invalid theme name".into());
    }
    fs::read_to_string(themes_dir().join(format!("{name}.json"))).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_theme(name: String, json: String) -> Result<(), String> {
    if !valid_name(&name) {
        return Err("invalid theme name (use letters, digits, - and _)".into());
    }
    Theme::from_json(&json).map_err(|e| e.to_string())?; // never save a broken theme
    fs::create_dir_all(themes_dir()).map_err(|e| e.to_string())?;
    fs::write(themes_dir().join(format!("{name}.json")), json).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_app_settings(state: tauri::State<AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_app_settings(
    state: tauri::State<AppState>,
    last_theme: String,
    brightness: u8,
    orientation: String,
    start_on_boot: bool,
) -> Result<(), String> {
    if !valid_name(&last_theme) {
        return Err("invalid theme name".into());
    }
    if !["landscape", "portrait", "rportrait", "rlandscape"].contains(&orientation.as_str()) {
        return Err("invalid orientation".into());
    }

    let settings = AppSettings { last_theme, brightness: brightness.min(100), orientation, start_on_boot };
    fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
    let path = settings_path();
    let temp_path = path.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&temp_path, json).map_err(|e| e.to_string())?;
    fs::rename(temp_path, path).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings.clone();
    sync_autostart(settings.start_on_boot)?;
    Ok(())
}

#[tauri::command]
fn snapshot(state: tauri::State<AppState>) -> Snapshot {
    state.sampler.lock().unwrap().sample()
}

/// Render the given theme JSON with live sensor values; returns a PNG data URL.
#[tauri::command]
fn preview(state: tauri::State<AppState>, theme_json: String) -> Result<String, String> {
    let theme = Theme::from_json(&theme_json).map_err(|e| e.to_string())?;
    let rd = Renderer::new(theme, &themes_dir()).map_err(|e| e.to_string())?;
    let snap = state.sampler.lock().unwrap().sample();
    let frame = rd.render(&snap).map_err(|e| e.to_string())?;
    let png = renderer::to_png(&frame).ok_or("PNG encoding failed")?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

fn slxd_path() -> PathBuf {
    // prefer a binary installed next to this app, then ~/.local/bin, then PATH
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("slxd");
            if p.exists() {
                return p;
            }
        }
    }
    if let Some(h) = std::env::var_os("HOME") {
        let p = PathBuf::from(h).join(".local/bin/slxd");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("slxd")
}

#[tauri::command]
fn daemon_start(
    state: tauri::State<AppState>,
    theme: String,
    brightness: u8,
    orientation: String,
    fps: f64,
) -> Result<(), String> {
    if !valid_name(&theme) {
        return Err("invalid theme name".into());
    }
    if !["landscape", "portrait", "rportrait", "rlandscape"].contains(&orientation.as_str()) {
        return Err("invalid orientation".into());
    }
    // Avoid competing with the optional legacy user service for the serial LCD.
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "slxd.service"])
        .status();
    let mut guard = state.daemon.lock().unwrap();
    if let Some(child) = guard.as_mut() {
        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
            return Err("daemon already running".into());
        }
    }
    let child = Command::new(slxd_path())
        .arg("--theme")
        .arg(themes_dir().join(format!("{theme}.json")))
        .args(["--brightness", &brightness.min(100).to_string()])
        .args(["--orientation", &orientation])
        .args(["--fps", &fps.clamp(0.2, 10.0).to_string()])
        .spawn()
        .map_err(|e| format!("cannot start slxd: {e}"))?;
    *guard = Some(child);
    Ok(())
}

#[tauri::command]
fn daemon_stop(state: tauri::State<AppState>) -> Result<(), String> {
    // Stop the optional user service too, otherwise it keeps streaming frames after the GUI stops.
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "slxd.service"])
        .status();
    if let Some(mut child) = state.daemon.lock().unwrap().take() {
        // SIGTERM first so slxd can exit cleanly, fall back to kill
        let pid = child.id() as i32;
        let _ = Command::new("kill").arg("-TERM").arg(pid.to_string()).status();
        std::thread::sleep(std::time::Duration::from_millis(400));
        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
            child.kill().map_err(|e| e.to_string())?;
        }
        let _ = child.wait();
    }
    Ok(())
}

#[tauri::command]
fn daemon_running(state: tauri::State<AppState>) -> bool {
    match state.daemon.lock().unwrap().as_mut() {
        Some(c) => matches!(c.try_wait(), Ok(None)),
        None => false,
    }
}

#[tauri::command]
fn list_ports() -> Vec<String> {
    lcd_core::discover::list_ports()
        .into_iter()
        .map(|p| {
            format!(
                "{}  {:04x}:{:04x}  {}",
                p.device.display(),
                p.vid.unwrap_or(0),
                p.pid.unwrap_or(0),
                p.serial.unwrap_or_default()
            )
        })
        .collect()
}

fn main() {
    let settings = load_app_settings();
    let _ = sync_autostart(settings.start_on_boot);
    tauri::Builder::default()
        .manage(AppState {
            sampler: Mutex::new(Sampler::new()),
            daemon: Mutex::new(None),
            settings: Mutex::new(settings),
        })
        .invoke_handler(tauri::generate_handler![
            list_themes,
            read_theme,
            save_theme,
            get_app_settings,
            save_app_settings,
            snapshot,
            preview,
            daemon_start,
            daemon_stop,
            daemon_running,
            list_ports
        ])
        .run(tauri::generate_context!())
        .expect("error while running the SLX LCD app");
}
