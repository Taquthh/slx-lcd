// SPDX-License-Identifier: GPL-3.0-or-later
//! Renders a JSON [`Theme`] with live sensor values into an RGBA frame for the LCD.

pub mod metrics;
pub mod theme;

use std::ffi::CString;
use std::path::{Path, PathBuf};

use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use lcd_core::Rgba;
use sensors::Snapshot;
use thiserror::Error;
use tiny_skia::{
    Color as SkColor, FillRule, LineCap, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

pub use theme::{Align, Background, Color, Theme, Widget};

const FONT_REGULAR: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/DejaVuSans-Bold.ttf");

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("cannot read {0}: {1}")]
    Io(PathBuf, std::io::Error),
    #[error("invalid theme: {0}")]
    Theme(#[from] serde_json::Error),
    #[error("cannot load image {0}: {1}")]
    Image(PathBuf, String),
    #[error("cannot create a {0}x{1} canvas")]
    Canvas(usize, usize),
}

pub struct Renderer {
    pub theme: Theme,
    background: Option<Pixmap>,
    images: Vec<Option<Pixmap>>, // parallel to theme.widgets
    regular: FontRef<'static>,
    bold: FontRef<'static>,
}

impl Renderer {
    /// Load a theme file; relative image paths resolve against its folder.
    pub fn from_file(path: &Path) -> Result<Self, RenderError> {
        let text = std::fs::read_to_string(path).map_err(|e| RenderError::Io(path.to_path_buf(), e))?;
        let theme = Theme::from_json(&text)?;
        Self::new(theme, path.parent().unwrap_or(Path::new(".")))
    }

    pub fn new(theme: Theme, base_dir: &Path) -> Result<Self, RenderError> {
        let load = |rel: &str| -> Result<Pixmap, RenderError> {
            let p = base_dir.join(rel);
            Pixmap::load_png(&p).map_err(|e| RenderError::Image(p, e.to_string()))
        };
        let background = match &theme.background.image {
            Some(rel) => Some(load(rel)?),
            None => None,
        };
        let mut images = Vec::with_capacity(theme.widgets.len());
        for w in &theme.widgets {
            images.push(match w {
                Widget::Image { path, .. } => Some(load(path)?),
                _ => None,
            });
        }
        Ok(Self {
            theme,
            background,
            images,
            regular: FontRef::try_from_slice(FONT_REGULAR).expect("bundled font"),
            bold: FontRef::try_from_slice(FONT_BOLD).expect("bundled font"),
        })
    }

    /// Render with the current wall-clock time.
    pub fn render(&self, snap: &Snapshot) -> Result<Rgba, RenderError> {
        let now = unsafe { libc::time(std::ptr::null_mut()) } as i64;
        self.render_at(snap, now)
    }

    /// Render using `unix_secs` for clock widgets (deterministic, for tests).
    pub fn render_at(&self, snap: &Snapshot, unix_secs: i64) -> Result<Rgba, RenderError> {
        let (w, h) = (self.theme.width, self.theme.height);
        let mut pm = Pixmap::new(w as u32, h as u32).ok_or(RenderError::Canvas(w, h))?;
        // opaque canvas: every pixel ends up with alpha 255
        pm.fill(SkColor::BLACK);

        if let Some(c) = self.theme.background.color {
            pm.fill(to_sk(c));
        }
        if let Some(bg) = &self.background {
            let sx = w as f32 / bg.width() as f32;
            let sy = h as f32 / bg.height() as f32;
            pm.draw_pixmap(0, 0, bg.as_ref(), &PixmapPaint::default(), Transform::from_scale(sx, sy), None);
        }

        for (i, widget) in self.theme.widgets.iter().enumerate() {
            match widget {
                Widget::Rect { x, y, w, h, radius, color } => fill_rrect(&mut pm, *x, *y, *w, *h, *radius, *color),
                Widget::Text { x, y, text, size, color, bold, align } => {
                    let s = metrics::render_template(text, snap);
                    self.text(&mut pm, *x, *y, &s, *size, *color, *bold, *align);
                }
                Widget::Bar { x, y, w, h, value, min, max, fg, bg, radius } => {
                    fill_rrect(&mut pm, *x, *y, *w, *h, *radius, *bg);
                    let f = fraction(metrics::resolve(snap, value), *min, *max);
                    if f > 0.0 {
                        // keep the filled part's corners round even when short
                        let fw = (*w * f as f32).max(h.min(*w * f as f32));
                        fill_rrect(&mut pm, *x, *y, fw.min(*w), *h, *radius, *fg);
                    }
                }
                Widget::Gauge { cx, cy, r, value, min, max, fg, bg, thickness, start, sweep } => {
                    arc(&mut pm, *cx, *cy, *r, *start, *sweep, *thickness, *bg);
                    let f = fraction(metrics::resolve(snap, value), *min, *max);
                    if f > 0.0 {
                        arc(&mut pm, *cx, *cy, *r, *start, *sweep * f as f32, *thickness, *fg);
                    }
                }
                Widget::Clock { x, y, format, size, color, bold, align } => {
                    let s = strftime(format, unix_secs);
                    self.text(&mut pm, *x, *y, &s, *size, *color, *bold, *align);
                }
                Widget::Image { x, y, w, h, .. } => {
                    if let Some(img) = &self.images[i] {
                        let sx = w.map(|w| w / img.width() as f32).unwrap_or(1.0);
                        let sy = h.map(|h| h / img.height() as f32).unwrap_or(1.0);
                        let t = Transform::from_row(sx, 0.0, 0.0, sy, *x, *y);
                        pm.draw_pixmap(0, 0, img.as_ref(), &PixmapPaint::default(), t, None);
                    }
                }
            }
        }

        Ok(Rgba::new(w, h, pm.data().to_vec()))
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&self, pm: &mut Pixmap, x: f32, y: f32, s: &str, size: f32, color: Color, bold: bool, align: Align) {
        let font = if bold { &self.bold } else { &self.regular };
        let scaled = font.as_scaled(PxScale::from(size));
        let width = text_width(&scaled, s);
        let x0 = match align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let baseline = y + scaled.ascent();
        let mut caret = x0;
        let mut prev = None;
        for ch in s.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(p) = prev {
                caret += scaled.kern(p, id);
            }
            let glyph = id.with_scale_and_position(PxScale::from(size), point(caret, baseline));
            caret += scaled.h_advance(id);
            prev = Some(id);
            if let Some(og) = font.outline_glyph(glyph) {
                let b = og.px_bounds();
                og.draw(|gx, gy, cov| {
                    let px = b.min.x as i32 + gx as i32;
                    let py = b.min.y as i32 + gy as i32;
                    blend(pm, px, py, color, cov);
                });
            }
        }
    }
}

fn text_width<F: Font, S: ScaleFont<F>>(scaled: &S, s: &str) -> f32 {
    let mut w = 0.0;
    let mut prev = None;
    for ch in s.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(p) = prev {
            w += scaled.kern(p, id);
        }
        w += scaled.h_advance(id);
        prev = Some(id);
    }
    w
}

fn blend(pm: &mut Pixmap, x: i32, y: i32, c: Color, coverage: f32) {
    if x < 0 || y < 0 || x >= pm.width() as i32 || y >= pm.height() as i32 {
        return;
    }
    let a = (coverage.clamp(0.0, 1.0)) * (c.3 as f32 / 255.0);
    if a <= 0.0 {
        return;
    }
    let i = ((y as u32 * pm.width() + x as u32) * 4) as usize;
    let d = pm.data_mut();
    let mix = |dst: u8, src: u8| (src as f32 * a + dst as f32 * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
    d[i] = mix(d[i], c.0);
    d[i + 1] = mix(d[i + 1], c.1);
    d[i + 2] = mix(d[i + 2], c.2);
    d[i + 3] = 255;
}

fn to_sk(c: Color) -> SkColor {
    SkColor::from_rgba8(c.0, c.1, c.2, c.3)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(to_sk(c));
    p.anti_alias = true;
    p
}

fn fraction(v: Option<f64>, min: f64, max: f64) -> f64 {
    match v {
        Some(v) if max > min => ((v - min) / (max - min)).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

fn fill_rrect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, radius: f32, c: Color) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let mut pb = PathBuilder::new();
    if r <= 0.0 {
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
    } else {
        pb.move_to(x + r, y);
        pb.line_to(x + w - r, y);
        pb.quad_to(x + w, y, x + w, y + r);
        pb.line_to(x + w, y + h - r);
        pb.quad_to(x + w, y + h, x + w - r, y + h);
        pb.line_to(x + r, y + h);
        pb.quad_to(x, y + h, x, y + h - r);
        pb.line_to(x, y + r);
        pb.quad_to(x, y, x + r, y);
    }
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
    }
}

#[allow(clippy::too_many_arguments)]
fn arc(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, start_deg: f32, sweep_deg: f32, thickness: f32, c: Color) {
    if sweep_deg <= 0.0 || r <= 0.0 {
        return;
    }
    let steps = ((sweep_deg / 2.0).ceil() as usize).max(2);
    let mut pb = PathBuilder::new();
    for i in 0..=steps {
        let a = (start_deg + sweep_deg * i as f32 / steps as f32).to_radians();
        let (px, py) = (cx + r * a.cos(), cy + r * a.sin());
        if i == 0 {
            pb.move_to(px, py);
        } else {
            pb.line_to(px, py);
        }
    }
    if let Some(path) = pb.finish() {
        let stroke = Stroke { width: thickness, line_cap: LineCap::Round, ..Default::default() };
        pm.stroke_path(&path, &paint(c), &stroke, Transform::identity(), None);
    }
}

/// strftime in the local time zone for `unix_secs`.
pub fn strftime(format: &str, unix_secs: i64) -> String {
    let Ok(fmt) = CString::new(format) else { return String::new() };
    let t = unix_secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    let mut buf = [0u8; 128];
    let n = unsafe { libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), fmt.as_ptr(), &tm) };
    String::from_utf8_lossy(&buf[..n]).to_string()
}

/// Encode an opaque RGBA frame as PNG (for previews).
pub fn to_png(frame: &Rgba) -> Option<Vec<u8>> {
    let size = tiny_skia::IntSize::from_wh(frame.width as u32, frame.height as u32)?;
    Pixmap::from_vec(frame.data.clone(), size)?.encode_png().ok()
}
