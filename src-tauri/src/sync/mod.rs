//! Executes a [`Plan`]: deletions, moves, then parallel downloads with cover
//! embedding. The manifest is saved as work completes so an interrupted sync
//! resumes where it stopped.

pub mod cover;
pub mod tagging;

use std::collections::{BTreeSet, HashMap};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::{stream, StreamExt};
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::sync::{OnceCell, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::diff::{Action, Desired, Plan};
use crate::error::{Error, Result};
use crate::manifest::{abs_path, Entry, Manifest};
use crate::settings::Settings;
use crate::subsonic::Client;

const SAVE_EVERY: usize = 10;
/// Cover is fetched a bit larger than embed size so the resize looks good.
const COVER_FETCH: u32 = 1000;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: String,
    pub done: usize,
    pub total: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current: Option<String>,
    pub current_cover: Option<String>,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub title: String,
    pub rel_path: String,
    pub error: String,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub downloaded: usize,
    pub moved: usize,
    pub deleted: usize,
    pub bytes: u64,
    pub failures: Vec<Failure>,
    pub warnings: Vec<String>,
    pub cancelled: bool,
}

pub struct Job {
    pub client: Arc<Client>,
    pub root: PathBuf,
    pub settings: Settings,
    pub plan: Plan,
    pub manifest: Manifest,
    pub selection: BTreeSet<String>,
    pub cancel: CancellationToken,
}

type CoverCache = Mutex<HashMap<String, Arc<OnceCell<Option<Arc<Vec<u8>>>>>>>;

struct Shared {
    client: Arc<Client>,
    root: PathBuf,
    tmp: PathBuf,
    settings: Settings,
    cancel: CancellationToken,
    covers: CoverCache,
    card_writes: Semaphore,
    bytes_done: AtomicU64,
    progress: Mutex<Progress>,
    warnings: Mutex<Vec<String>>,
    seq: AtomicUsize,
}

pub async fn run<F>(job: Job, on_progress: F) -> Result<Outcome>
where
    F: Fn(Progress) + Send + Sync + 'static,
{
    let Job {
        client,
        root,
        settings,
        plan,
        mut manifest,
        selection,
        cancel,
    } = job;

    manifest.version = crate::manifest::VERSION;
    manifest.server = client.base_url().to_string();
    manifest.selection = selection;
    let root_for_save = root.clone();
    let save = move |m: &Manifest| m.save(&root_for_save);
    save(&manifest)?;

    let mut outcome = Outcome::default();
    let mut deletes = Vec::new();
    let mut moves = Vec::new();
    let mut downloads = Vec::new();
    for t in plan.tracks {
        match t.action {
            Action::Delete => deletes.push(t),
            Action::Move { .. } => moves.push(t),
            Action::Download { .. } => downloads.push(t),
            Action::Keep => {}
        }
    }

    let shared = Arc::new(Shared {
        client,
        root: root.clone(),
        tmp: std::env::temp_dir().join(format!("echotransfer-{}", std::process::id())),
        settings: settings.clone(),
        cancel: cancel.clone(),
        covers: Mutex::new(HashMap::new()),
        card_writes: Semaphore::new(2),
        bytes_done: AtomicU64::new(0),
        progress: Mutex::new(Progress {
            phase: "preparing".into(),
            total: deletes.len() + moves.len() + downloads.len(),
            bytes_total: downloads.iter().map(|t| t.bytes).sum(),
            ..Default::default()
        }),
        warnings: Mutex::new(Vec::new()),
        seq: AtomicUsize::new(0),
    });

    // Progress ticker: UI gets updates at a steady rate, not per chunk.
    let on_progress = Arc::new(on_progress);
    let ticker = {
        let shared = shared.clone();
        let on_progress = on_progress.clone();
        tokio::spawn(async move {
            loop {
                on_progress(shared.snapshot());
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
    };

    let result: Result<()> = async {
        // 1. Deletions first: frees space.
        shared.set_phase("removing");
        for t in &deletes {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            shared.set_current(Some(t.rel_path.clone()), None);
            match remove_owned(&root, &t.rel_path) {
                Ok(()) => {
                    manifest.entries.remove(&t.song_id);
                    outcome.deleted += 1;
                }
                Err(e) => outcome.failures.push(Failure {
                    title: t.rel_path.clone(),
                    rel_path: t.rel_path.clone(),
                    error: e.to_string(),
                }),
            }
            shared.step();
        }
        save(&manifest)?;

        // 2. Renames (path template changed).
        shared.set_phase("moving");
        for t in &moves {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            let Action::Move { from } = &t.action else { unreachable!() };
            let res = (|| -> Result<()> {
                let dest = abs_path(&root, &t.rel_path);
                if let Some(p) = dest.parent() {
                    std::fs::create_dir_all(p)?;
                }
                std::fs::rename(abs_path(&root, from), &dest)?;
                prune_empty_dirs(&root, &abs_path(&root, from));
                Ok(())
            })();
            match res {
                Ok(()) => {
                    if let Some(e) = manifest.entries.get_mut(&t.song_id) {
                        e.rel_path = t.rel_path.clone();
                    }
                    outcome.moved += 1;
                }
                Err(e) => outcome.failures.push(Failure {
                    title: t.rel_path.clone(),
                    rel_path: t.rel_path.clone(),
                    error: e.to_string(),
                }),
            }
            shared.step();
        }
        save(&manifest)?;

        // 3. Downloads.
        shared.set_phase("downloading");
        std::fs::create_dir_all(&shared.tmp)?;
        let concurrency = settings.concurrency.max(1);
        let mut results = stream::iter(downloads.into_iter().map(|t| {
            let shared = shared.clone();
            async move {
                let d = t.desired.clone().expect("download has desired");
                let replaces = match &t.action {
                    Action::Download { replaces } => replaces.clone(),
                    _ => None,
                };
                let r = shared.download_one(&d, replaces.as_deref()).await;
                (d, r)
            }
        }))
        .buffer_unordered(concurrency);

        let mut since_save = 0;
        while let Some((d, r)) = results.next().await {
            match r {
                Ok(entry) => {
                    outcome.downloaded += 1;
                    outcome.bytes += entry.local_size;
                    manifest.entries.insert(d.song.id.clone(), entry);
                    since_save += 1;
                    if since_save >= SAVE_EVERY {
                        save(&manifest)?;
                        since_save = 0;
                    }
                }
                Err(Error::Cancelled) => {}
                Err(e) => {
                    shared.progress.lock().unwrap().failed += 1;
                    outcome.failures.push(Failure {
                        title: format!("{} – {}", d.fields.artist, d.fields.title),
                        rel_path: d.rel_path.clone(),
                        error: e.to_string(),
                    });
                }
            }
            shared.step();
        }
        drop(results);
        save(&manifest)?;
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }
    .await;

    // Always: persist manifest, cleanup.
    let _ = save(&manifest);
    let _ = std::fs::remove_dir_all(&shared.tmp);
    if settings.clean_apple_double {
        shared.set_phase("cleaning");
        let tops: BTreeSet<String> = manifest
            .entries
            .values()
            .filter_map(|e| e.rel_path.split('/').next().map(str::to_string))
            .collect();
        for top in tops {
            clean_apple_double(&root.join(top));
        }
    }
    ticker.abort();
    shared.set_phase("done");
    on_progress(shared.snapshot());
    outcome.warnings = std::mem::take(&mut *shared.warnings.lock().unwrap());

    match result {
        Ok(()) => Ok(outcome),
        Err(Error::Cancelled) => {
            outcome.cancelled = true;
            Ok(outcome)
        }
        Err(e) => Err(e),
    }
}

impl Shared {
    fn snapshot(&self) -> Progress {
        let mut p = self.progress.lock().unwrap().clone();
        p.bytes_done = self.bytes_done.load(Ordering::Relaxed);
        p
    }

    fn set_phase(&self, phase: &str) {
        self.progress.lock().unwrap().phase = phase.into();
    }

    fn set_current(&self, current: Option<String>, cover: Option<String>) {
        let mut p = self.progress.lock().unwrap();
        p.current = current;
        p.current_cover = cover;
    }

    fn step(&self) {
        self.progress.lock().unwrap().done += 1;
    }

    fn warn(&self, msg: String) {
        eprintln!("warning: {msg}");
        self.warnings.lock().unwrap().push(msg);
    }

    async fn cover_for(&self, cover_id: &str) -> Option<Arc<Vec<u8>>> {
        let cell = self
            .covers
            .lock()
            .unwrap()
            .entry(cover_id.to_string())
            .or_default()
            .clone();
        cell.get_or_init(|| async {
            let raw = match self.client.cover_art(cover_id, COVER_FETCH).await {
                Ok(b) => b,
                Err(e) => {
                    self.warn(format!("cover {cover_id}: {e}"));
                    return None;
                }
            };
            let max = self.settings.cover_size;
            match tokio::task::spawn_blocking(move || cover::normalize(&raw, max)).await {
                Ok(Ok(jpeg)) => Some(Arc::new(jpeg)),
                Ok(Err(e)) => {
                    self.warn(format!("cover {cover_id}: {e}"));
                    None
                }
                Err(e) => {
                    self.warn(format!("cover {cover_id}: {e}"));
                    None
                }
            }
        })
        .await
        .clone()
    }

    async fn download_one(&self, d: &Desired, replaces: Option<&str>) -> Result<Entry> {
        if self.cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.set_current(
            Some(format!("{} – {}", d.fields.artist, d.fields.title)),
            d.song.cover_art.clone(),
        );

        let cover = match &d.song.cover_art {
            Some(id) => self.cover_for(id).await,
            None => None,
        };

        // Download to local temp file.
        let ext = d.rel_path.rsplit_once('.').map(|(_, e)| e).unwrap_or("bin");
        let tmp = self
            .tmp
            .join(format!("{}.{ext}", self.seq.fetch_add(1, Ordering::Relaxed)));
        let transcode = self.settings.profile.transcode();
        let res = self.fetch_to(&tmp, &d.song.id, transcode).await;
        if let Err(e) = res {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e);
        }

        // Embed cover (+ tags for transcodes) off the async runtime.
        let fields = transcode.is_some().then(|| d.fields.clone());
        let tag_path = tmp.clone();
        let tag_res = tokio::task::spawn_blocking(move || {
            tagging::apply(&tag_path, cover.as_deref().map(|c| c.as_slice()), fields.as_ref())
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))?;
        if let Err(e) = tag_res {
            self.warn(format!("{}: could not embed cover: {e}", d.rel_path));
        }

        // Copy to card: .part + fsync + rename. Limited concurrency.
        let _permit = self.card_writes.acquire().await.expect("semaphore open");
        if self.cancel.is_cancelled() {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(Error::Cancelled);
        }
        let dest = abs_path(&self.root, &d.rel_path);
        let local_size = {
            let tmp = tmp.clone();
            let dest = dest.clone();
            tokio::task::spawn_blocking(move || write_to_card(&tmp, &dest))
                .await
                .map_err(|e| Error::Other(e.to_string()))??
        };
        let _ = tokio::fs::remove_file(&tmp).await;

        if let Some(old) = replaces.filter(|o| *o != d.rel_path) {
            if let Err(e) = remove_owned(&self.root, old) {
                self.warn(format!("{old}: could not remove old file: {e}"));
            }
        }

        Ok(Entry {
            rel_path: d.rel_path.clone(),
            album_id: d.album_id.clone(),
            fingerprint: d.fingerprint.clone(),
            profile: self.settings.profile.key().into(),
            cover_art: d.song.cover_art.clone(),
            cover_size: self.settings.cover_size,
            local_size,
            synced_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        })
    }

    async fn fetch_to(&self, path: &Path, song_id: &str, transcode: Option<(&str, u32)>) -> Result<()> {
        let resp = self
            .client
            .media_request(song_id, transcode)
            .send()
            .await?
            .error_for_status()?;
        let ctype = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if ctype.contains("json") || ctype.contains("xml") {
            let body = resp.text().await.unwrap_or_default();
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v["subsonic-response"]["error"]["message"].as_str().map(str::to_string))
                .unwrap_or(body);
            return Err(Error::Other(if transcode.is_some() {
                format!("Server could not convert this track ({msg}). Check that ffmpeg is installed for Navidrome, or use original files.")
            } else {
                format!("Server refused download: {msg}")
            }));
        }
        let mut file = tokio::fs::File::create(path).await?;
        let mut body = resp.bytes_stream();
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => return Err(Error::Cancelled),
                chunk = body.next() => match chunk {
                    Some(c) => {
                        let c = c?;
                        file.write_all(&c).await?;
                        self.bytes_done.fetch_add(c.len() as u64, Ordering::Relaxed);
                    }
                    None => break,
                }
            }
        }
        file.flush().await?;
        Ok(())
    }
}

/// Plain byte copy (no xattrs → no `._` files on exFAT), fsync, atomic rename.
fn write_to_card(src: &Path, dest: &Path) -> Result<u64> {
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut part_name = dest.file_name().unwrap_or_default().to_os_string();
    part_name.push(".part");
    let part = dest.with_file_name(part_name);
    let res = (|| -> Result<u64> {
        let mut input = std::fs::File::open(src)?;
        let out = std::fs::File::create(&part)?;
        let mut w = BufWriter::with_capacity(1 << 20, out);
        let n = std::io::copy(&mut input, &mut w)?;
        w.flush()?;
        w.get_ref().sync_all()?;
        drop(w);
        std::fs::rename(&part, dest)?;
        Ok(n)
    })();
    if res.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    res
}

/// Remove a manifest-owned file (missing is fine) and prune empty parents.
fn remove_owned(root: &Path, rel: &str) -> Result<()> {
    let p = abs_path(root, rel);
    match std::fs::remove_file(&p) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    prune_empty_dirs(root, &p);
    Ok(())
}

/// Walk up from `file`'s parent removing directories that are empty
/// (ignoring macOS junk files), stopping at `root`.
fn prune_empty_dirs(root: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(d) = dir {
        if d == root || !d.starts_with(root) {
            break;
        }
        let Ok(entries) = std::fs::read_dir(d) else { break };
        let names: Vec<_> = entries.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        if !names.iter().all(|n| is_junk(n)) {
            break;
        }
        for n in &names {
            let _ = std::fs::remove_file(d.join(n));
        }
        if std::fs::remove_dir(d).is_err() {
            break;
        }
        dir = d.parent();
    }
}

fn is_junk(name: &str) -> bool {
    name == ".DS_Store" || name.starts_with("._")
}

/// Remove AppleDouble `._*` files, which players can show as broken tracks.
fn clean_apple_double(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            clean_apple_double(&e.path());
        } else if name.starts_with("._") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_to_card_is_atomic_and_creates_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        std::fs::write(&src, b"hello").unwrap();
        let dest = dir.path().join("card/Music/A/x.mp3");
        assert_eq!(write_to_card(&src, &dest).unwrap(), 5);
        assert_eq!(std::fs::read(&dest).unwrap(), b"hello");
        assert!(!dir.path().join("card/Music/A/x.mp3.part").exists());
    }

    #[test]
    fn remove_owned_prunes_empty_parents_but_not_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Music/A/B")).unwrap();
        std::fs::write(root.join("Music/A/B/x.mp3"), b"x").unwrap();
        std::fs::write(root.join("Music/A/B/._x.mp3"), b"x").unwrap();
        std::fs::write(root.join("Music/keep.txt"), b"x").unwrap();
        remove_owned(root, "Music/A/B/x.mp3").unwrap();
        assert!(!root.join("Music/A").exists());
        assert!(root.join("Music/keep.txt").exists());
        // missing file is fine
        remove_owned(root, "Music/nope.mp3").unwrap();
        assert!(root.exists());
    }

    #[test]
    fn apple_double_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/._song.mp3"), b"x").unwrap();
        std::fs::write(dir.path().join("a/b/song.mp3"), b"x").unwrap();
        clean_apple_double(dir.path());
        assert!(!dir.path().join("a/b/._song.mp3").exists());
        assert!(dir.path().join("a/b/song.mp3").exists());
    }
}
