mod diff;
mod error;
mod library;
mod manifest;
mod paths;
mod settings;
mod subsonic;
mod sync;
mod volumes;

#[cfg(test)]
mod e2e_tests;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use diff::{AlbumDiff, Plan, Status, Summary};
use error::{Error, Result};
use library::{ArtistView, Library};
use manifest::{abs_path, Manifest};
use settings::Settings;
use subsonic::Client;
use volumes::Volume;

#[derive(Default)]
struct AppState {
    settings: Mutex<Settings>,
    client: RwLock<Option<Arc<Client>>>,
    library: RwLock<Option<Arc<Library>>>,
    /// Last computed plan, for per-track statuses in the album detail view.
    last_plan: Mutex<Option<Plan>>,
    sync_cancel: Mutex<Option<CancellationToken>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerInfo {
    url: String,
    username: String,
    server: String,
    /// Authenticated getCoverArt URL; UI appends `&id=..&size=..`.
    cover_art_base: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CardInfo {
    has_manifest: bool,
    selection: Vec<String>,
    track_count: usize,
    server: String,
    same_server: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffResponse {
    summary: Summary,
    albums: HashMap<String, AlbumDiff>,
    available_bytes: u64,
    total_bytes: u64,
    projected_free: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TrackRow {
    id: String,
    title: String,
    artist: String,
    track: Option<u32>,
    disc: Option<u32>,
    duration: u64,
    size: u64,
    suffix: String,
    status: Option<Status>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncDone {
    outcome: Option<sync::Outcome>,
    error: Option<String>,
}

async fn client(state: &AppState) -> Result<Arc<Client>> {
    state.client.read().await.clone().ok_or(Error::NotConnected)
}

async fn library(state: &AppState) -> Result<Arc<Library>> {
    state
        .library
        .read()
        .await
        .clone()
        .ok_or_else(|| Error::Other("Library not loaded".into()))
}

fn server_info(c: &Client, username: &str, server: String) -> ServerInfo {
    let cover = c.endpoint_url("getCoverArt", &[]);
    ServerInfo {
        url: c.base_url().to_string(),
        username: username.to_string(),
        server,
        cover_art_base: cover.to_string(),
    }
}

fn card_root(mount: &str) -> Result<PathBuf> {
    let p = PathBuf::from(mount);
    if !p.is_dir() {
        return Err(Error::Other(format!("{mount} is not mounted")));
    }
    Ok(p)
}

#[tauri::command]
async fn connect(
    state: State<'_, AppState>,
    url: String,
    username: String,
    password: String,
    remember: bool,
) -> Result<ServerInfo> {
    let c = Client::new(&url, &username, &password)?;
    let server = c.ping().await?;
    let base = c.base_url().to_string();
    if remember {
        if let Err(e) = settings::store_password(&base, &username, &password) {
            eprintln!("keychain: {e}");
        }
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.server_url = base;
        s.username = username.clone();
        s.save()?;
    }
    let info = server_info(&c, &username, server);
    *state.client.write().await = Some(Arc::new(c));
    *state.library.write().await = None;
    Ok(info)
}

/// Reconnect with saved credentials, if any.
#[tauri::command]
async fn auto_connect(state: State<'_, AppState>) -> Result<Option<ServerInfo>> {
    let (url, user) = {
        let s = state.settings.lock().unwrap();
        (s.server_url.clone(), s.username.clone())
    };
    if url.is_empty() || user.is_empty() {
        return Ok(None);
    }
    let Some(pw) = settings::load_password(&url, &user) else {
        return Ok(None);
    };
    let c = Client::new(&url, &user, &pw)?;
    let server = c.ping().await?;
    let info = server_info(&c, &user, server);
    *state.client.write().await = Some(Arc::new(c));
    Ok(Some(info))
}

#[tauri::command]
async fn disconnect(state: State<'_, AppState>) -> Result<()> {
    {
        let s = state.settings.lock().unwrap();
        settings::forget_password(&s.server_url, &s.username);
    }
    *state.client.write().await = None;
    *state.library.write().await = None;
    *state.last_plan.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings> {
    let mut s = state.settings.lock().unwrap();
    let new = Settings {
        server_url: s.server_url.clone(),
        username: s.username.clone(),
        ..settings
    }
    .sanitized();
    new.save()?;
    *s = new.clone();
    Ok(new)
}

#[tauri::command]
fn list_volumes() -> Vec<Volume> {
    volumes::list()
}

#[tauri::command]
async fn load_library(state: State<'_, AppState>, refresh: bool) -> Result<Vec<ArtistView>> {
    if !refresh {
        if let Some(lib) = state.library.read().await.clone() {
            return Ok(lib.artists());
        }
    }
    let c = client(&state).await?;
    let lib = Arc::new(Library::fetch(&c).await?);
    *state.library.write().await = Some(lib.clone());
    Ok(lib.artists())
}

#[tauri::command]
async fn card_info(state: State<'_, AppState>, mount: String) -> Result<CardInfo> {
    let root = card_root(&mount)?;
    let m = Manifest::load(&root)?;
    let current = match state.client.read().await.as_ref() {
        Some(c) => c.base_url().to_string(),
        None => String::new(),
    };
    Ok(CardInfo {
        has_manifest: Manifest::exists(&root),
        selection: m.selection.iter().cloned().collect(),
        track_count: m.entries.len(),
        same_server: m.server.is_empty() || m.server == current,
        server: m.server,
    })
}

fn build_plan(lib: &Library, root: &Path, manifest: &Manifest, settings: &Settings, selection: &BTreeSet<String>) -> Plan {
    let desired = diff::desired_tracks(lib, selection, manifest, settings, |rel| abs_path(root, rel).exists());
    diff::compute(desired, manifest, settings, |rel| {
        std::fs::metadata(abs_path(root, rel)).ok().map(|m| m.len())
    })
}

#[tauri::command]
async fn compute_diff(state: State<'_, AppState>, mount: String, selection: Vec<String>) -> Result<DiffResponse> {
    let root = card_root(&mount)?;
    let lib = library(&state).await?;
    let settings = state.settings.lock().unwrap().clone();
    let selection: BTreeSet<String> = selection.into_iter().collect();
    let vol = volumes::find(&mount);

    let (plan, summary) = tokio::task::spawn_blocking(move || -> Result<(Plan, Summary)> {
        let manifest = Manifest::load(&root)?;
        let plan = build_plan(&lib, &root, &manifest, &settings, &selection);
        let summary = plan.summary(&manifest);
        Ok((plan, summary))
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))??;

    let (available, total) = vol.map(|v| (v.available_bytes, v.total_bytes)).unwrap_or((0, 0));
    let albums = plan.albums();
    *state.last_plan.lock().unwrap() = Some(plan);
    Ok(DiffResponse {
        projected_free: available as i64 - summary.download_bytes as i64 + summary.freed_bytes as i64,
        summary,
        albums,
        available_bytes: available,
        total_bytes: total,
    })
}

#[tauri::command]
async fn album_tracks(state: State<'_, AppState>, album_id: String) -> Result<Vec<TrackRow>> {
    let lib = library(&state).await?;
    let plan = state.last_plan.lock().unwrap();
    Ok(lib
        .songs(&album_id)
        .iter()
        .map(|s| TrackRow {
            id: s.id.clone(),
            title: s.title.clone(),
            artist: s.artist.clone().unwrap_or_default(),
            track: s.track,
            disc: s.disc_number,
            duration: s.duration,
            size: s.size,
            suffix: s.suffix.clone(),
            status: plan.as_ref().and_then(|p| p.status_of(&s.id)),
        })
        .collect())
}

#[tauri::command]
async fn start_sync(app: AppHandle, state: State<'_, AppState>, mount: String, selection: Vec<String>) -> Result<()> {
    let root = card_root(&mount)?;
    let c = client(&state).await?;
    let lib = library(&state).await?;
    let settings = state.settings.lock().unwrap().clone();
    let selection: BTreeSet<String> = selection.into_iter().collect();

    let cancel = {
        let mut slot = state.sync_cancel.lock().unwrap();
        if slot.is_some() {
            return Err(Error::Other("A sync is already running".into()));
        }
        let t = CancellationToken::new();
        *slot = Some(t.clone());
        t
    };

    let prepared = {
        let root = root.clone();
        let settings = settings.clone();
        let selection = selection.clone();
        tokio::task::spawn_blocking(move || -> Result<(Manifest, Plan, Summary)> {
            let manifest = Manifest::load(&root)?;
            let plan = build_plan(&lib, &root, &manifest, &settings, &selection);
            let summary = plan.summary(&manifest);
            Ok((manifest, plan, summary))
        })
        .await
        .map_err(|e| Error::Other(e.to_string()))
        .and_then(|r| r)
    };
    let (manifest, plan, summary) = match prepared {
        Ok(p) => p,
        Err(e) => {
            *state.sync_cancel.lock().unwrap() = None;
            return Err(e);
        }
    };
    if let Some(v) = volumes::find(&mount) {
        let projected = v.available_bytes as i64 - summary.download_bytes as i64 + summary.freed_bytes as i64;
        if projected < 0 {
            *state.sync_cancel.lock().unwrap() = None;
            return Err(Error::Other(format!(
                "Not enough space on {}: need {} MB more",
                v.name,
                (-projected) / 1_000_000
            )));
        }
    }

    let job = sync::Job {
        client: c,
        root,
        settings,
        plan,
        manifest,
        selection,
        cancel,
    };
    tauri::async_runtime::spawn(async move {
        let progress_app = app.clone();
        let res = sync::run(job, move |p| {
            let _ = progress_app.emit("sync-progress", p);
        })
        .await;
        app.state::<AppState>().sync_cancel.lock().unwrap().take();
        let done = match res {
            Ok(o) => SyncDone { outcome: Some(o), error: None },
            Err(e) => SyncDone { outcome: None, error: Some(e.to_string()) },
        };
        let _ = app.emit("sync-done", done);
    });
    Ok(())
}

#[tauri::command]
fn cancel_sync(state: State<'_, AppState>) {
    if let Some(t) = state.sync_cancel.lock().unwrap().as_ref() {
        t.cancel();
    }
}

/// Polls mounted volumes and notifies the UI when cards are inserted/ejected.
fn watch_volumes(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last: Vec<String> = Vec::new();
        loop {
            let vols = tokio::task::spawn_blocking(volumes::list).await.unwrap_or_default();
            let mounts: Vec<String> = vols.iter().map(|v| v.mount_point.clone()).collect();
            if mounts != last {
                last = mounts;
                let _ = app.emit("volumes-changed", vols);
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(Settings::load().sanitized()),
            ..Default::default()
        })
        .setup(|app| {
            watch_volumes(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            connect,
            auto_connect,
            disconnect,
            get_settings,
            save_settings,
            list_volumes,
            load_library,
            card_info,
            compute_diff,
            album_tracks,
            start_sync,
            cancel_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
