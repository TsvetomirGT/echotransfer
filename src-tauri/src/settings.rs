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
    // Explicit names: kebab-case alone gives "mp3320", but keys and the UI use "mp3-320".
    #[serde(rename = "mp3-320", alias = "mp3320")]
    Mp3320,
    #[serde(rename = "mp3-256", alias = "mp3256")]
    Mp3256,
    #[serde(rename = "mp3-192", alias = "mp3192")]
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

    /// MP3 profile for a bitrate (192, 256, anything else = 320).
    pub fn mp3(bitrate: u32) -> Self {
        match bitrate {
            192 => Profile::Mp3192,
            256 => Profile::Mp3256,
            _ => Profile::Mp3320,
        }
    }

    pub fn bitrate(self) -> Option<u32> {
        self.transcode().map(|(_, b)| b)
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
    /// Bitrate used when the format is MP3; kept while the format is Original.
    pub mp3_bitrate: u32,
    /// Max edge of embedded cover in px.
    pub cover_size: u32,
    pub concurrency: usize,
    pub path_template: String,
    pub clean_apple_double: bool,
    /// Fill {group} from the work tag (TIT1) when there is no grouping tag.
    pub work_as_group: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            username: String::new(),
            profile: Profile::Original,
            mp3_bitrate: 320,
            cover_size: 500,
            concurrency: 4,
            path_template: DEFAULT_TEMPLATE.into(),
            clean_apple_double: true,
            work_as_group: false,
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
        // Profile wins: older settings files only have the profile.
        self.mp3_bitrate = self.profile.bitrate().unwrap_or(self.mp3_bitrate);
        self.mp3_bitrate = Profile::mp3(self.mp3_bitrate).bitrate().unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_take_bitrate_from_profile() {
        let s: Settings = serde_json::from_str(r#"{"profile":"mp3-192"}"#).unwrap();
        let s = s.sanitized();
        assert_eq!(s.profile, Profile::Mp3192);
        assert_eq!(s.mp3_bitrate, 192);
    }

    #[test]
    fn profile_serializes_as_key() {
        for p in [Profile::Original, Profile::Mp3320, Profile::Mp3256, Profile::Mp3192] {
            assert_eq!(serde_json::to_value(p).unwrap(), p.key());
        }
    }

    #[test]
    fn bitrate_kept_while_original() {
        let s = Settings { mp3_bitrate: 256, ..Settings::default() }.sanitized();
        assert_eq!(s.profile, Profile::Original);
        assert_eq!(s.mp3_bitrate, 256);
        let s = Settings { mp3_bitrate: 7, ..Settings::default() }.sanitized();
        assert_eq!(s.mp3_bitrate, 320);
    }
}
