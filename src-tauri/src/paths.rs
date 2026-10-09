//! Card-side path rendering + FAT32/exFAT-safe sanitizing.
//!
//! Template syntax: `{field}` or `{field:02}` (zero-padded number). Text in
//! `[...]` is dropped entirely if any field inside it is empty.
//! Fields: albumartist, artist, album, title, year, track, disc, genre.
//! `disc` is only set for multi-disc albums. The extension is appended.

use unicode_normalization::UnicodeNormalization;

use crate::subsonic::Song;

const MAX_COMPONENT_BYTES: usize = 120;

#[derive(Debug, Default, Clone)]
pub struct TrackFields {
    pub albumartist: String,
    pub artist: String,
    pub album: String,
    pub title: String,
    pub genre: String,
    pub year: Option<u32>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
}

impl TrackFields {
    pub fn from_song(song: &Song, album_artist: Option<&str>, multi_disc: bool) -> Self {
        let artist = song.artist.clone().filter(|s| !s.is_empty());
        let albumartist = album_artist
            .map(str::to_string)
            .or_else(|| song.display_album_artist.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| artist.clone())
            .unwrap_or_else(|| "Unknown Artist".into());
        Self {
            artist: artist.unwrap_or_else(|| albumartist.clone()),
            albumartist,
            album: song.album.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "Unknown Album".into()),
            title: if song.title.is_empty() { song.id.clone() } else { song.title.clone() },
            genre: song.genre.clone().unwrap_or_default(),
            year: song.year.filter(|y| *y > 0),
            track: song.track.filter(|t| *t > 0),
            disc: if multi_disc { song.disc_number.filter(|d| *d > 0) } else { None },
        }
    }

    fn value(&self, name: &str, pad: usize) -> Option<String> {
        let num = |n: Option<u32>| n.map(|n| format!("{n:0pad$}"));
        let text = |s: &str| (!s.is_empty()).then(|| s.replace(['/', '\\'], "-"));
        match name {
            "albumartist" => text(&self.albumartist),
            "artist" => text(&self.artist),
            "album" => text(&self.album),
            "title" => text(&self.title),
            "genre" => text(&self.genre),
            "year" => num(self.year),
            "track" => num(self.track),
            "disc" => num(self.disc),
            _ => None,
        }
    }
}

/// Renders a relative path (forward slashes) for a track.
pub fn render(template: &str, fields: &TrackFields, ext: &str) -> String {
    let raw = expand(template, fields);
    let mut parts: Vec<String> = raw
        .split('/')
        .map(|p| sanitize_component(p, MAX_COMPONENT_BYTES))
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        parts.push(sanitize_component(&fields.title, MAX_COMPONENT_BYTES));
    }
    let ext = sanitize_component(&ext.to_lowercase(), 10);
    if let Some(last) = parts.last_mut() {
        if !ext.is_empty() {
            last.push('.');
            last.push_str(&ext);
        }
    }
    parts.join("/")
}

fn expand(template: &str, fields: &TrackFields) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                let mut group = String::new();
                for g in chars.by_ref() {
                    if g == ']' {
                        break;
                    }
                    group.push(g);
                }
                if let Some(s) = expand_fields(&group, fields) {
                    out.push_str(&s);
                }
            }
            '{' => {
                let mut name = String::new();
                for g in chars.by_ref() {
                    if g == '}' {
                        break;
                    }
                    name.push(g);
                }
                out.push_str(&field(&name, fields).unwrap_or_default());
            }
            _ => out.push(c),
        }
    }
    out
}

/// Expands placeholders in `s`; None if any of them is empty.
fn expand_fields(s: &str, fields: &TrackFields) -> Option<String> {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '{' {
            let name: String = chars.by_ref().take_while(|g| *g != '}').collect();
            out.push_str(&field(&name, fields)?);
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn field(spec: &str, fields: &TrackFields) -> Option<String> {
    let (name, pad) = match spec.split_once(':') {
        Some((n, p)) => (n, p.parse().unwrap_or(0)),
        None => (spec, 0),
    };
    fields.value(name.trim(), pad)
}

pub fn sanitize_component(s: &str, max_bytes: usize) -> String {
    let cleaned: String = s
        .nfc()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect();
    let mut out = trim_edges(&cleaned).to_string();
    if out.len() > max_bytes {
        let mut cut = max_bytes;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out = trim_edges(&out).to_string();
    }
    let upper = out.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    let reserved = matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    if reserved {
        out.push('_');
    }
    out
}

/// Strip whitespace, leading dots (hidden files, `._` clashes) and trailing dots.
fn trim_edges(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace())
        .trim_start_matches('.')
        .trim_end_matches(|c: char| c == '.' || c.is_whitespace())
        .trim_start()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::DEFAULT_TEMPLATE;

    fn fields() -> TrackFields {
        TrackFields {
            albumartist: "Radiohead".into(),
            artist: "Radiohead".into(),
            album: "OK Computer".into(),
            title: "Paranoid Android".into(),
            genre: "Rock".into(),
            year: Some(1997),
            track: Some(2),
            disc: None,
        }
    }

    #[test]
    fn default_template() {
        assert_eq!(
            render(DEFAULT_TEMPLATE, &fields(), "FLAC"),
            "Music/Radiohead/1997 - OK Computer/02 Paranoid Android.flac"
        );
    }

    #[test]
    fn optional_groups_drop_when_empty() {
        let mut f = fields();
        f.year = None;
        f.disc = Some(2);
        assert_eq!(
            render(DEFAULT_TEMPLATE, &f, "mp3"),
            "Music/Radiohead/OK Computer/2-02 Paranoid Android.mp3"
        );
    }

    #[test]
    fn missing_track_number_trims_leading_space() {
        let mut f = fields();
        f.track = None;
        assert!(render(DEFAULT_TEMPLATE, &f, "mp3").ends_with("/Paranoid Android.mp3"));
    }

    #[test]
    fn slashes_in_values_do_not_create_dirs() {
        let mut f = fields();
        f.albumartist = "AC/DC".into();
        f.title = "What?: \"Yes\" *".into();
        assert_eq!(
            render(DEFAULT_TEMPLATE, &f, "mp3"),
            "Music/AC-DC/1997 - OK Computer/02 What__ _Yes_ _.mp3"
        );
    }

    #[test]
    fn sanitize_edges_and_reserved() {
        assert_eq!(sanitize_component("  ...hidden. ", 120), "hidden");
        assert_eq!(sanitize_component("CON", 120), "CON_");
        assert_eq!(sanitize_component("com1.txt", 120), "com1.txt_");
        assert_eq!(sanitize_component("Comet", 120), "Comet");
        assert_eq!(sanitize_component("a\u{0}b\tc", 120), "abc");
    }

    #[test]
    fn truncates_on_char_boundary() {
        let long = "é".repeat(100); // 200 bytes
        let s = sanitize_component(&long, 121);
        assert_eq!(s.len(), 120);
        assert!(s.chars().all(|c| c == 'é'));
    }

    #[test]
    fn nfc_normalized() {
        let decomposed = "Beyonce\u{0301}";
        assert_eq!(sanitize_component(decomposed, 120), "Beyonc\u{e9}");
    }
}
