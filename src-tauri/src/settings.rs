//! User settings, persisted as JSON next to the executable (portable).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, fs, path::Path};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Glass panel opacity, 0 (pure floating text) to 100.
    pub opacity: u8,
    pub font_px: u32,
    pub font_color: String,
    /// Colour of the karaoke sweep.
    pub fill_color: String,
    pub outline: bool,
    pub outline_color: String,
    pub outline_width: f32,
    /// sans | display | serif | mono
    pub font_family: String,
    /// left | center | right
    pub align: String,
    /// Visible lyric lines: 1, 3 or 5.
    pub lines: u8,
    pub karaoke: bool,
    pub click_through: bool,
    pub always_on_top: bool,
    pub first_run_done: bool,
    pub window: Option<Geometry>,
    /// Per-song timing offsets in milliseconds, keyed by "title|artist".
    pub offsets: HashMap<String, i64>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            opacity: 35,
            font_px: 30,
            font_color: "#ffffff".into(),
            fill_color: "#7dd3fc".into(),
            outline: true,
            outline_color: "#000000".into(),
            outline_width: 2.0,
            font_family: "sans".into(),
            align: "center".into(),
            lines: 3,
            karaoke: true,
            click_through: false,
            always_on_top: true,
            first_run_done: false,
            window: None,
            offsets: HashMap::new(),
        }
    }
}

fn valid_color(s: &str) -> bool {
    s.starts_with('#')
        && matches!(s.len(), 4 | 7 | 9)
        && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

impl Settings {
    fn sanitize(&mut self) {
        let d = Settings::default();
        self.opacity = self.opacity.min(100);
        self.font_px = self.font_px.clamp(12, 120);
        self.outline_width = if self.outline_width.is_finite() {
            self.outline_width.clamp(0.0, 8.0)
        } else {
            d.outline_width
        };
        self.lines = match self.lines {
            0 | 1 => 1,
            2 | 3 => 3,
            _ => 5,
        };
        if !["sans", "display", "serif", "mono"].contains(&self.font_family.as_str()) {
            self.font_family = d.font_family.clone();
        }
        if !["left", "center", "right"].contains(&self.align.as_str()) {
            self.align = d.align.clone();
        }
        // Colours end up inside CSS, so only accept plain hex values.
        if !valid_color(&self.font_color) {
            self.font_color = d.font_color.clone();
        }
        if !valid_color(&self.fill_color) {
            self.fill_color = d.fill_color.clone();
        }
        if !valid_color(&self.outline_color) {
            self.outline_color = d.outline_color.clone();
        }
        for v in self.offsets.values_mut() {
            *v = (*v).clamp(-30_000, 30_000);
        }
    }

    /// Shallow-merge a JSON patch (camelCase keys) into the settings.
    pub fn merge(&mut self, patch: Value) -> Result<(), String> {
        let mut cur = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        if let (Some(c), Some(p)) = (cur.as_object_mut(), patch.as_object()) {
            for (k, v) in p {
                c.insert(k.clone(), v.clone());
            }
        }
        let mut next: Settings = serde_json::from_value(cur).map_err(|e| e.to_string())?;
        next.sanitize();
        *self = next;
        Ok(())
    }
}

pub fn load(dir: &Path) -> Settings {
    let mut s: Settings = fs::read_to_string(dir.join("settings.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    s.sanitize();
    s
}

pub fn save(dir: &Path, s: &Settings) {
    let Ok(text) = serde_json::to_string_pretty(s) else { return };
    let tmp = dir.join("settings.json.tmp");
    if fs::write(&tmp, text).is_ok() {
        let _ = fs::rename(&tmp, dir.join("settings.json"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_clamps_and_validates() {
        let mut s = Settings::default();
        s.merge(json!({"fontPx": 500, "opacity": 0, "fontColor": "red; x", "lines": 4}))
            .unwrap();
        assert_eq!(s.font_px, 120);
        assert_eq!(s.opacity, 0);
        assert_eq!(s.font_color, "#ffffff");
        assert_eq!(s.lines, 5);
    }

    #[test]
    fn old_files_with_missing_keys_still_load() {
        let s: Settings = serde_json::from_str(r#"{"opacity": 10}"#).unwrap();
        assert_eq!(s.opacity, 10);
        assert_eq!(s.font_px, 30);
    }
}
