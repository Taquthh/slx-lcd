// SPDX-License-Identifier: GPL-3.0-or-later
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl Color {
    pub fn parse(s: &str) -> Option<Color> {
        let h = s.trim().strip_prefix('#')?;
        let v = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            6 => Some(Color(v(0)?, v(2)?, v(4)?, 255)),
            8 => Some(Color(v(0)?, v(2)?, v(4)?, v(6)?)),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("invalid color {s:?}, use #rrggbb or #rrggbbaa")))
    }
}

impl Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let Color(r, g, b, a) = *self;
        if a == 255 {
            s.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}"))
        } else {
            s.serialize_str(&format!("#{r:02x}{g:02x}{b:02x}{a:02x}"))
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

fn d_white() -> Color {
    Color(255, 255, 255, 255)
}
fn d_track() -> Color {
    Color(255, 255, 255, 36)
}
fn d_size() -> f32 {
    24.0
}
fn d_max() -> f64 {
    100.0
}
fn d_start() -> f32 {
    135.0
}
fn d_sweep() -> f32 {
    270.0
}
fn d_thick() -> f32 {
    14.0
}
fn d_fmt() -> String {
    "%H:%M".into()
}
fn d_media_fps() -> f32 {
    2.0
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Widget {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default)]
        radius: f32,
        #[serde(default = "d_track")]
        color: Color,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        #[serde(default = "d_size")]
        size: f32,
        #[serde(default = "d_white")]
        color: Color,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        align: Align,
    },
    Bar {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        value: String,
        #[serde(default)]
        min: f64,
        #[serde(default = "d_max")]
        max: f64,
        #[serde(default = "d_white")]
        fg: Color,
        #[serde(default = "d_track")]
        bg: Color,
        #[serde(default)]
        radius: f32,
    },
    Gauge {
        cx: f32,
        cy: f32,
        r: f32,
        value: String,
        #[serde(default)]
        min: f64,
        #[serde(default = "d_max")]
        max: f64,
        #[serde(default = "d_white")]
        fg: Color,
        #[serde(default = "d_track")]
        bg: Color,
        #[serde(default = "d_thick")]
        thickness: f32,
        /// angle in degrees, 0 = 3 o'clock, clockwise (screen coordinates)
        #[serde(default = "d_start")]
        start: f32,
        #[serde(default = "d_sweep")]
        sweep: f32,
    },
    Clock {
        x: f32,
        y: f32,
        /// strftime format, e.g. "%H:%M:%S" or "%d %b %Y"
        #[serde(default = "d_fmt")]
        format: String,
        #[serde(default = "d_size")]
        size: f32,
        #[serde(default = "d_white")]
        color: Color,
        #[serde(default)]
        bold: bool,
        #[serde(default)]
        align: Align,
    },
    Image {
        x: f32,
        y: f32,
        /// PNG path, relative to the theme file
        path: String,
        #[serde(default)]
        w: Option<f32>,
        #[serde(default)]
        h: Option<f32>,
        #[serde(default)]
        frames: Vec<String>,
        #[serde(default = "d_media_fps")]
        fps: f32,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Background {
    #[serde(default)]
    pub color: Option<Color>,
    /// PNG path (relative to the theme file), stretched to the canvas
    #[serde(default)]
    pub image: Option<String>,
    /// Optional PNG frames for an animated image or video background
    #[serde(default)]
    pub frames: Vec<String>,
    #[serde(default = "d_media_fps")]
    pub fps: f32,
}

impl Default for Background {
    fn default() -> Self {
        Background { color: Some(Color(8, 12, 20, 255)), image: None, frames: Vec::new(), fps: d_media_fps() }
    }
}

fn d_dim() -> usize {
    480
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Theme {
    pub name: String,
    #[serde(default = "d_dim")]
    pub width: usize,
    #[serde(default = "d_dim")]
    pub height: usize,
    #[serde(default)]
    pub background: Background,
    #[serde(default)]
    pub widgets: Vec<Widget>,
}

impl Theme {
    pub fn from_json(text: &str) -> Result<Theme, serde_json::Error> {
        serde_json::from_str(text)
    }
}
