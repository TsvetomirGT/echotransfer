use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::Result;

pub const DEFAULT_TEMPLATE: &str = "Music/{albumartist}/[{year} - ]{album}/[{disc}-]{track:02} {title}";
const KEYRING_SERVICE: &str = "com.echotransfer.app";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    #[default]
    Original,
    Mp3320,
    Mp3256,
    Mp3192,
}

impl Profile {
    /// Server-side transcode format + bitrate, or None for original file.
    pub fn transcode(self) -> Option<(&'static str, u32)> {
        match self {
            Profile::Original => None,
            Profile::Mp3320 => Some(("mp3", 320)),
            Profile::Mp3256 => Some(("mp3", 256)),
            Profile::Mp3192 => Some(("mp3", 192)),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Profile::Original => "original",
            Profile::Mp3320 => "mp3-320",
            Profile::Mp3256 => "mp3-256",
            Profile::Mp3192 => "mp3-192",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub server_url: String,
    pub username: String,
    pub profile: Profile,
    /// Max edge of embedded cover in px.
    pub cover_size: u32,
    pub concurrency: usize,
    pub path_template: String,
    pub clean_apple_double: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            username: String::new(),
            profile: Profile::Original,
            cover_size: 500,
            concurrency: 4,
            path_template: DEFAULT_TEMPLATE.into(),
            clean_apple_double: true,
        }
    }
}

fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("echotransfer")
        .join("settings.json")
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read(settings_path())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let p = settings_path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(p, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn sanitized(mut self) -> Self {
        self.cover_size = self.cover_size.clamp(100, 1500);
        self.concurrency = self.concurrency.clamp(1, 16);
        if self.path_template.trim().is_empty() {
            self.path_template = DEFAULT_TEMPLATE.into();
        }
        self
    }
}

fn keyring_entry(url: &str, user: &str) -> Result<keyring::Entry> {
    Ok(keyring::Entry::new(KEYRING_SERVICE, &format!("{user}@{url}"))?)
}

pub fn store_password(url: &str, user: &str, password: &str) -> Result<()> {
    keyring_entry(url, user)?.set_password(password)?;
    Ok(())
}

pub fn load_password(url: &str, user: &str) -> Option<String> {
    keyring_entry(url, user).ok()?.get_password().ok()
}

pub fn forget_password(url: &str, user: &str) {
    if let Ok(e) = keyring_entry(url, user) {
        let _ = e.delete_credential();
    }
}
