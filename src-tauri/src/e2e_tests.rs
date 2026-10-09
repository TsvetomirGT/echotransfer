//! End-to-end sync against a real Navidrome server. Opt-in:
//! ECHO_E2E_URL=http://127.0.0.1:4533 ECHO_E2E_USER=admin ECHO_E2E_PASS=... cargo test e2e -- --ignored

use std::collections::BTreeSet;
use std::sync::Arc;

use lofty::prelude::*;
use tokio_util::sync::CancellationToken;

use crate::diff::{self, Action};
use crate::library::Library;
use crate::manifest::{abs_path, Manifest};
use crate::settings::{Profile, Settings};
use crate::subsonic::Client;
use crate::sync;

fn client() -> Client {
    let url = std::env::var("ECHO_E2E_URL").expect("ECHO_E2E_URL");
    let user = std::env::var("ECHO_E2E_USER").unwrap_or_else(|_| "admin".into());
    let pass = std::env::var("ECHO_E2E_PASS").expect("ECHO_E2E_PASS");
    Client::new(&url, &user, &pass).unwrap()
}

async fn sync_once(c: &Arc<Client>, lib: &Library, root: &std::path::Path, s: &Settings, sel: &BTreeSet<String>) -> sync::Outcome {
    let manifest = Manifest::load(root).unwrap();
    let desired = diff::desired_tracks(lib, sel, &manifest, s, |r| abs_path(root, r).exists());
    let plan = diff::compute(desired, &manifest, s, |r| std::fs::metadata(abs_path(root, r)).ok().map(|m| m.len()));
    let job = sync::Job {
        client: c.clone(),
        root: root.to_path_buf(),
        settings: s.clone(),
        plan,
        manifest,
        selection: sel.clone(),
        cancel: CancellationToken::new(),
    };
    sync::run(job, |_| {}).await.unwrap()
}

#[tokio::test]
#[ignore]
async fn e2e_full_sync_cycle() {
    let c = Arc::new(client());
    println!("server: {}", c.ping().await.unwrap());
    let lib = Library::fetch(&c).await.unwrap();
    let all: BTreeSet<String> = lib.albums.keys().cloned().collect();
    let total: usize = all.iter().map(|a| lib.songs(a).len()).sum();
    assert!(total > 0);

    let card = tempfile::tempdir().unwrap();
    let root = card.path();
    let s = Settings::default();

    // 1. Fresh sync copies everything with embedded covers.
    let o = sync_once(&c, &lib, root, &s, &all).await;
    println!("first: {o:?}");
    assert_eq!(o.downloaded, total);
    assert!(o.failures.is_empty(), "{:?}", o.failures);
    let m = Manifest::load(root).unwrap();
    assert_eq!(m.entries.len(), total);
    for e in m.entries.values() {
        let p = abs_path(root, &e.rel_path);
        assert!(p.exists(), "{}", e.rel_path);
        assert!(e.rel_path.starts_with("Music/"));
        let f = lofty::read_from_path(&p).unwrap();
        let tag = f.primary_tag().expect("tag");
        assert_eq!(tag.pictures().len(), 1, "cover in {}", e.rel_path);
        let img = image::load_from_memory(tag.pictures()[0].data()).unwrap();
        assert!(img.width() <= 500 && img.height() <= 500);
        assert!(!tag.pictures()[0].data().windows(2).any(|w| w == [0xFF, 0xC2]), "baseline jpeg");
    }

    // 2. Second sync is a no-op.
    let o = sync_once(&c, &lib, root, &s, &all).await;
    assert_eq!((o.downloaded, o.deleted, o.moved), (0, 0, 0));

    // 3. Deleted file on card gets restored.
    let victim = m.entries.values().next().unwrap().rel_path.clone();
    std::fs::remove_file(abs_path(root, &victim)).unwrap();
    let o = sync_once(&c, &lib, root, &s, &all).await;
    assert_eq!(o.downloaded, 1);

    // 4. Template change only renames.
    let s2 = Settings { path_template: "{albumartist}/{album}/{track:02} {title}".into(), ..s.clone() };
    let o = sync_once(&c, &lib, root, &s2, &all).await;
    assert_eq!((o.downloaded, o.moved), (0, total));
    assert!(!root.join("Music").exists(), "old dirs pruned");

    // 5. Deselect one album → its tracks deleted, dirs pruned.
    let drop_album = all.iter().next().unwrap().clone();
    let n_drop = lib.songs(&drop_album).len();
    let mut fewer = all.clone();
    fewer.remove(&drop_album);
    let o = sync_once(&c, &lib, root, &s2, &fewer).await;
    assert_eq!(o.deleted, n_drop);
    assert_eq!(Manifest::load(root).unwrap().entries.len(), total - n_drop);

    // 6. Transcode profile: re-downloads as mp3 with text tags.
    let s3 = Settings { profile: Profile::Mp3192, ..s2.clone() };
    let o = sync_once(&c, &lib, root, &s3, &fewer).await;
    println!("transcode: {o:?}");
    let m = Manifest::load(root).unwrap();
    if !o.failures.is_empty() {
        // Server without ffmpeg: failures must be explained, originals kept.
        assert!(o.failures.iter().all(|f| f.error.contains("ffmpeg")), "{:?}", o.failures);
        assert_eq!(m.entries.len(), total - n_drop);
        assert!(m.entries.values().all(|e| abs_path(root, &e.rel_path).exists()));
        println!("server cannot transcode; skipping final no-op check");
        return;
    }
    assert!(m.entries.values().all(|e| e.rel_path.ends_with(".mp3")));
    let any = abs_path(root, &m.entries.values().next().unwrap().rel_path);
    let f = lofty::read_from_path(&any).unwrap();
    assert!(f.primary_tag().unwrap().title().is_some());

    // Plan sanity: nothing left to do.
    let manifest = Manifest::load(root).unwrap();
    let desired = diff::desired_tracks(&lib, &fewer, &manifest, &s3, |r| abs_path(root, r).exists());
    let plan = diff::compute(desired, &manifest, &s3, |r| std::fs::metadata(abs_path(root, r)).ok().map(|m| m.len()));
    assert!(plan.tracks.iter().all(|t| t.action == Action::Keep));
}
