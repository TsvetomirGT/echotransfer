//! Compares what the selection wants on the card with what the manifest says is there.

use std::collections::{BTreeSet, HashMap, HashSet};

use md5::{Digest, Md5};
use serde::Serialize;

use crate::library::Library;
use crate::manifest::Manifest;
use crate::paths::{self, TrackFields};
use crate::settings::Settings;
use crate::subsonic::Song;

/// Rough overhead for an embedded cover when estimating transfer size.
const COVER_OVERHEAD: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    New,
    Updated,
    Removed,
    Synced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Download; `replaces` is the old card path to delete afterwards if it differs.
    Download { replaces: Option<String> },
    /// Metadata unchanged, only path template changed: rename on card.
    Move { from: String },
    Delete,
    Keep,
}

/// A track the current selection wants on the card.
#[derive(Debug, Clone)]
pub struct Desired {
    pub song: Song,
    pub album_id: String,
    pub fields: TrackFields,
    pub rel_path: String,
    pub fingerprint: String,
    pub est_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct TrackPlan {
    pub song_id: String,
    pub album_id: String,
    pub rel_path: String,
    pub status: Status,
    pub action: Action,
    /// Bytes to download (Download) or bytes freed (Delete).
    pub bytes: u64,
    /// Present for everything except Delete.
    pub desired: Option<Desired>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub new: usize,
    pub updated: usize,
    pub removed: usize,
    pub synced: usize,
    pub download_bytes: u64,
    pub freed_bytes: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumDiff {
    pub status: Option<Status>,
    pub new: usize,
    pub updated: usize,
    pub removed: usize,
    pub synced: usize,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub tracks: Vec<TrackPlan>,
}

pub fn fingerprint(s: &Song) -> String {
    let parts = [
        s.size.to_string(),
        s.suffix.clone(),
        s.title.clone(),
        s.artist.clone().unwrap_or_default(),
        s.display_album_artist.clone().unwrap_or_default(),
        s.album.clone().unwrap_or_default(),
        s.track.unwrap_or(0).to_string(),
        s.disc_number.unwrap_or(0).to_string(),
        s.year.unwrap_or(0).to_string(),
        s.genre.clone().unwrap_or_default(),
        s.duration.to_string(),
    ];
    hex::encode(Md5::digest(parts.join("\u{1f}")))
}

/// Builds the desired card layout for the selected albums.
/// `occupied(rel_path)` reports files on the card that this app does not own.
pub fn desired_tracks(
    lib: &Library,
    selection: &BTreeSet<String>,
    manifest: &Manifest,
    settings: &Settings,
    occupied: impl Fn(&str) -> bool,
) -> Vec<Desired> {
    let owned: HashSet<String> = manifest.entries.values().map(|e| e.rel_path.to_lowercase()).collect();
    let mut taken: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for album_id in selection {
        let songs = lib.songs(album_id);
        let multi_disc = songs.iter().any(|s| s.disc_number.unwrap_or(1) > 1);
        let album_artist = lib.albums.get(album_id).and_then(|a| a.artist.as_deref());
        for song in songs {
            let fields = TrackFields::from_song(song, album_artist, multi_disc);
            let ext = match settings.profile.transcode() {
                Some((fmt, _)) => fmt.to_string(),
                None if song.suffix.is_empty() => "mp3".into(),
                None => song.suffix.clone(),
            };
            let base = paths::render(&settings.path_template, &fields, &ext);
            // FAT/exFAT are case-insensitive: compare lowercased.
            let mut rel_path = base.clone();
            let mut n = 2;
            while taken.contains(&rel_path.to_lowercase())
                || (!owned.contains(&rel_path.to_lowercase()) && occupied(&rel_path))
            {
                rel_path = with_suffix(&base, n);
                n += 1;
            }
            taken.insert(rel_path.to_lowercase());
            let est_bytes = match settings.profile.transcode() {
                None => song.size,
                Some((_, kbps)) => song.duration * kbps as u64 * 1000 / 8,
            } + COVER_OVERHEAD;
            out.push(Desired {
                song: song.clone(),
                album_id: album_id.clone(),
                fields,
                rel_path,
                fingerprint: fingerprint(song),
                est_bytes,
            });
        }
    }
    out
}

fn with_suffix(path: &str, n: usize) -> String {
    match path.rsplit_once('.') {
        Some((stem, ext)) if !stem.ends_with('/') => format!("{stem} ({n}).{ext}"),
        _ => format!("{path} ({n})"),
    }
}

/// `local_size(rel_path)` returns the size of a file on the card, if it exists.
pub fn compute(
    desired: Vec<Desired>,
    manifest: &Manifest,
    settings: &Settings,
    local_size: impl Fn(&str) -> Option<u64>,
) -> Plan {
    let profile = settings.profile.key();
    let mut tracks = Vec::with_capacity(desired.len());
    let wanted: HashSet<String> = desired.iter().map(|d| d.song.id.clone()).collect();

    for d in desired {
        let (status, action, bytes) = match manifest.entries.get(&d.song.id) {
            None => (Status::New, Action::Download { replaces: None }, d.est_bytes),
            Some(e) => {
                let intact = local_size(&e.rel_path) == Some(e.local_size);
                let content_same = e.fingerprint == d.fingerprint
                    && e.profile == profile
                    && e.cover_art == d.song.cover_art
                    && e.cover_size == settings.cover_size;
                let replaces = (e.rel_path != d.rel_path).then(|| e.rel_path.clone());
                if !intact || !content_same {
                    (Status::Updated, Action::Download { replaces }, d.est_bytes)
                } else if let Some(from) = replaces {
                    (Status::Updated, Action::Move { from }, 0)
                } else {
                    (Status::Synced, Action::Keep, 0)
                }
            }
        };
        tracks.push(TrackPlan {
            song_id: d.song.id.clone(),
            album_id: d.album_id.clone(),
            rel_path: d.rel_path.clone(),
            status,
            action,
            bytes,
            desired: Some(d),
        });
    }

    for (id, e) in &manifest.entries {
        if !wanted.contains(id) {
            tracks.push(TrackPlan {
                song_id: id.clone(),
                album_id: e.album_id.clone(),
                rel_path: e.rel_path.clone(),
                status: Status::Removed,
                action: Action::Delete,
                bytes: local_size(&e.rel_path).unwrap_or(0),
                desired: None,
            });
        }
    }
    Plan { tracks }
}

impl Plan {
    pub fn summary(&self, manifest: &Manifest) -> Summary {
        let mut s = Summary::default();
        for t in &self.tracks {
            match t.status {
                Status::New => s.new += 1,
                Status::Updated => s.updated += 1,
                Status::Removed => s.removed += 1,
                Status::Synced => s.synced += 1,
            }
            match &t.action {
                Action::Download { .. } => {
                    s.download_bytes += t.bytes;
                    if let Some(e) = manifest.entries.get(&t.song_id) {
                        s.freed_bytes += e.local_size;
                    }
                }
                Action::Delete => s.freed_bytes += t.bytes,
                _ => {}
            }
        }
        s
    }

    pub fn albums(&self) -> HashMap<String, AlbumDiff> {
        let mut map: HashMap<String, AlbumDiff> = HashMap::new();
        for t in &self.tracks {
            let a = map.entry(t.album_id.clone()).or_default();
            match t.status {
                Status::New => a.new += 1,
                Status::Updated => a.updated += 1,
                Status::Removed => a.removed += 1,
                Status::Synced => a.synced += 1,
            }
        }
        for a in map.values_mut() {
            a.status = Some(if a.new + a.updated > 0 {
                if a.updated == 0 && a.synced == 0 && a.removed == 0 {
                    Status::New
                } else {
                    Status::Updated
                }
            } else if a.removed > 0 {
                if a.synced > 0 { Status::Updated } else { Status::Removed }
            } else {
                Status::Synced
            });
        }
        map
    }

    pub fn status_of(&self, song_id: &str) -> Option<Status> {
        self.tracks.iter().find(|t| t.song_id == song_id).map(|t| t.status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Entry;
    use crate::settings::Profile;
    use crate::subsonic::Album;

    fn song(id: &str, album: &str, track: u32) -> Song {
        Song {
            id: id.into(),
            title: format!("Song {id}"),
            album: Some("Album".into()),
            album_id: Some(album.into()),
            artist: Some("Artist".into()),
            track: Some(track),
            cover_art: Some(format!("al-{album}")),
            size: 1000,
            suffix: "flac".into(),
            duration: 100,
            ..Default::default()
        }
    }

    fn lib(songs: Vec<Song>) -> Library {
        let albums = vec![Album {
            id: "a1".into(),
            name: "Album".into(),
            artist: Some("Artist".into()),
            ..Default::default()
        }];
        Library::from_parts(albums, songs)
    }

    fn sel(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    /// Manifest as if `plan` had been synced perfectly.
    fn synced_manifest(desired: &[Desired], settings: &Settings) -> Manifest {
        let mut m = Manifest::default();
        for d in desired {
            m.entries.insert(
                d.song.id.clone(),
                Entry {
                    rel_path: d.rel_path.clone(),
                    album_id: d.album_id.clone(),
                    fingerprint: d.fingerprint.clone(),
                    profile: settings.profile.key().into(),
                    cover_art: d.song.cover_art.clone(),
                    cover_size: settings.cover_size,
                    local_size: 1100,
                    synced_at: 0,
                },
            );
        }
        m
    }

    fn run(lib: &Library, m: &Manifest, s: &Settings, local: impl Fn(&str) -> Option<u64>) -> Plan {
        let d = desired_tracks(lib, &sel(&["a1"]), m, s, |_| false);
        compute(d, m, s, local)
    }

    #[test]
    fn fresh_card_everything_new() {
        let l = lib(vec![song("s1", "a1", 1), song("s2", "a1", 2)]);
        let s = Settings::default();
        let p = run(&l, &Manifest::default(), &s, |_| None);
        assert!(p.tracks.iter().all(|t| t.status == Status::New));
        let sum = p.summary(&Manifest::default());
        assert_eq!(sum.new, 2);
        assert_eq!(sum.download_bytes, 2 * (1000 + COVER_OVERHEAD));
        assert_eq!(p.albums()["a1"].status, Some(Status::New));
    }

    #[test]
    fn synced_card_nothing_to_do() {
        let l = lib(vec![song("s1", "a1", 1)]);
        let s = Settings::default();
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);
        let p = run(&l, &m, &s, |_| Some(1100));
        assert_eq!(p.tracks[0].status, Status::Synced);
        assert_eq!(p.tracks[0].action, Action::Keep);
        assert_eq!(p.summary(&m).download_bytes, 0);
    }

    #[test]
    fn missing_or_changed_local_file_redownloads() {
        let l = lib(vec![song("s1", "a1", 1)]);
        let s = Settings::default();
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);
        for local in [None, Some(5)] {
            let p = run(&l, &m, &s, |_| local);
            assert_eq!(p.tracks[0].status, Status::Updated);
            assert_eq!(p.tracks[0].action, Action::Download { replaces: None });
        }
    }

    #[test]
    fn server_change_redownloads() {
        let s = Settings::default();
        let l = lib(vec![song("s1", "a1", 1)]);
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);

        let mut changed = song("s1", "a1", 1);
        changed.size = 2000;
        let p = run(&lib(vec![changed]), &m, &s, |_| Some(1100));
        assert_eq!(p.tracks[0].status, Status::Updated);

        let mut new_cover = song("s1", "a1", 1);
        new_cover.cover_art = Some("other".into());
        let p = run(&lib(vec![new_cover]), &m, &s, |_| Some(1100));
        assert_eq!(p.tracks[0].status, Status::Updated);
    }

    #[test]
    fn retag_moves_path_and_replaces_old_file() {
        let s = Settings::default();
        let l = lib(vec![song("s1", "a1", 1)]);
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);
        let mut retitled = song("s1", "a1", 1);
        retitled.title = "New Title".into();
        let p = run(&lib(vec![retitled]), &m, &s, |_| Some(1100));
        assert_eq!(
            p.tracks[0].action,
            Action::Download { replaces: Some(d[0].rel_path.clone()) }
        );
    }

    #[test]
    fn template_change_only_moves() {
        let s = Settings::default();
        let l = lib(vec![song("s1", "a1", 1)]);
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);
        let s2 = Settings {
            path_template: "{artist}/{title}".into(),
            ..Settings::default()
        };
        let p = run(&l, &m, &s2, |_| Some(1100));
        assert_eq!(p.tracks[0].action, Action::Move { from: d[0].rel_path.clone() });
        assert_eq!(p.tracks[0].rel_path, "Artist/Song s1.flac");
    }

    #[test]
    fn profile_change_redownloads_with_new_ext() {
        let s = Settings::default();
        let l = lib(vec![song("s1", "a1", 1)]);
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);
        let s2 = Settings {
            profile: Profile::Mp3320,
            ..Settings::default()
        };
        let p = run(&l, &m, &s2, |_| Some(1100));
        assert!(p.tracks[0].rel_path.ends_with(".mp3"));
        assert!(matches!(p.tracks[0].action, Action::Download { replaces: Some(_) }));
        assert_eq!(p.tracks[0].bytes, 100 * 320 * 1000 / 8 + COVER_OVERHEAD);
    }

    #[test]
    fn deselected_and_server_removed_are_deleted() {
        let s = Settings::default();
        let l = lib(vec![song("s1", "a1", 1), song("s2", "a1", 2)]);
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |_| false);
        let m = synced_manifest(&d, &s);

        // s2 gone from server
        let p = run(&lib(vec![song("s1", "a1", 1)]), &m, &s, |_| Some(1100));
        let del: Vec<_> = p.tracks.iter().filter(|t| t.action == Action::Delete).collect();
        assert_eq!(del.len(), 1);
        assert_eq!(del[0].song_id, "s2");
        assert_eq!(p.albums()["a1"].status, Some(Status::Updated));

        // album deselected
        let none = desired_tracks(&l, &sel(&[]), &m, &s, |_| false);
        let p = compute(none, &m, &s, |_| Some(1100));
        assert!(p.tracks.iter().all(|t| t.status == Status::Removed));
        assert_eq!(p.summary(&m).freed_bytes, 2200);
        assert_eq!(p.albums()["a1"].status, Some(Status::Removed));
    }

    #[test]
    fn collisions_get_suffixes_and_foreign_files_are_avoided() {
        let s = Settings::default();
        let mut a = song("s1", "a1", 1);
        let mut b = song("s2", "a1", 1);
        a.title = "Same".into();
        b.title = "SAME".into();
        let l = lib(vec![a, b]);
        let d = desired_tracks(&l, &sel(&["a1"]), &Manifest::default(), &s, |p| p.to_lowercase().ends_with("01 same.flac"));
        assert!(d[0].rel_path.ends_with("01 Same (2).flac"));
        assert!(d[1].rel_path.ends_with("01 SAME (3).flac"));
    }
}
