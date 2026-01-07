use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const MAX_RECENT_PROJECTS: usize = 10;

/// Default margin for panel content (in virtual units, scaled by ui_scale)
pub const DEFAULT_PANEL_MARGIN: f32 = 8.0;

pub fn default_ui_scale() -> f32 {
    1.0
}

/// App configuration stored on disk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub recent_projects: Vec<String>,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            recent_projects: Vec::new(),
            ui_scale: 1.0,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA").ok().map(|appdata| {
                PathBuf::from(appdata)
                    .join("PixelSpriteStudio")
                    .join("config.json")
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::env::var("HOME").ok().map(|home| {
                PathBuf::from(home)
                    .join(".config")
                    .join("pixel-sprite-studio")
                    .join("config.json")
            })
        }
    }

    pub fn load() -> Self {
        Self::config_path()
            .and_then(|path| fs::read_to_string(&path).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            // Create parent directories if needed
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = fs::write(&path, json);
            }
        }
    }

    pub fn add_recent(&mut self, path: &str) {
        // Remove if already exists (to move to front)
        self.recent_projects.retain(|p| p != path);
        // Add to front
        self.recent_projects.insert(0, path.to_string());
        // Trim to max size
        self.recent_projects.truncate(MAX_RECENT_PROJECTS);
        self.save();
    }

    pub fn remove_recent(&mut self, path: &str) {
        self.recent_projects.retain(|p| p != path);
        self.save();
    }
}
