//! User settings, persisted as JSON in the app config dir
//! (`~/.config/io.olund.poe2overlay/settings.json`). Only the UI language for now.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::trade::Lang;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// UI language code (`"ru"`, `"de"`, …); `None`/`"auto"` = follow the game client.
    #[serde(default)]
    pub language: Option<String>,
}

impl Settings {
    /// The chosen UI language, `None` = auto.
    pub fn lang(&self) -> Option<Lang> {
        self.language.as_deref().and_then(Lang::from_code)
    }
}

/// What the settings panel shows: the choice and the language actually in effect.
#[derive(Debug, Clone, Serialize)]
pub struct SettingsView {
    /// `"auto"` or a language code.
    pub language: String,
    /// The UI language in effect (auto resolves to the game client's).
    pub effective: String,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

/// Saved settings; defaults when missing or unreadable.
pub fn load(app: &AppHandle) -> Settings {
    path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> std::io::Result<()> {
    let p = path(app).ok_or_else(|| std::io::Error::other("no app config dir"))?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(p, json)
}
