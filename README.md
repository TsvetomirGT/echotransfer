# EchoTransfer

Copy music from your [Navidrome](https://www.navidrome.org/) server to the SD card of a **Snowsky Echo** or **Echo Mini**, and keep the card in sync.

Pick the artists and albums you want on the player, see exactly what will change on the card, and press **Sync**. Only new or changed tracks are copied, so the second sync takes seconds, not hours. Cover art is embedded into every track, so it shows up on the player.

- **Selective sync:** choose whole artists or single albums. Your choice is saved on the card itself.
- **See changes before syncing:** every album is marked *New*, *Updated*, *Remove* or *On card*, with a bar showing how much space the card will have afterwards.
- **Fast re-syncs:** unchanged tracks are skipped. Renamed folders are moved on the card instead of downloaded again.
- **Covers that the Echo can show:** the Echo ignores `cover.jpg` files in folders. EchoTransfer embeds the album cover into each track as a small, standard JPEG that the player can display.
- **Safe:** EchoTransfer only deletes files it put on the card itself. Your own files on the card are never touched.
- **Optional MP3 conversion** to fit more music on a small card.
- Works on **macOS, Windows and Linux**.

## Contents

- [Download and install](#download-and-install)
- [What you need](#what-you-need)
- [How to use it](#how-to-use-it)
- [Settings](#settings)
- [How syncing works](#how-syncing-works)
- [Troubleshooting](#troubleshooting)
- [Building from source](#building-from-source)

## Download and install

Download the latest version from the [**Releases page**](https://github.com/TsvetomirGT/echotransfer/releases/latest).

| System | File to download |
| --- | --- |
| macOS, Apple Silicon (M1 and newer) | `EchoTransfer_x.y.z_aarch64.dmg` |
| macOS, Intel | `EchoTransfer_x.y.z_x64.dmg` |
| Windows 10/11 | `EchoTransfer_x.y.z_x64-setup.exe` (or the `.msi`) |
| Linux | `.AppImage` (any distribution), `.deb` (Debian, Ubuntu) or `.rpm` (Fedora, openSUSE) |

Not sure which Mac you have? Open  menu → **About This Mac**. "Chip: Apple M…" means Apple Silicon. "Processor: Intel" means Intel.

### First launch

The app isn't code-signed yet, so your system will warn you the first time you open it.

**macOS**

1. Open the `.dmg` and drag **EchoTransfer** into **Applications**.
2. Open it. If macOS says the app "can't be opened" or "is damaged", open **Terminal** and run:
   ```sh
   xattr -cr /Applications/EchoTransfer.app
   ```
   Then open the app again. Alternatively, go to **System Settings → Privacy & Security**, scroll down, and click **Open Anyway**.

**Windows**

1. Run the installer.
2. If Windows SmartScreen shows "Windows protected your PC", click **More info → Run anyway**.

**Linux**

- **AppImage:** make it executable, then run it:
  ```sh
  chmod +x EchoTransfer_*.AppImage
  ./EchoTransfer_*.AppImage
  ```
- **.deb:** `sudo apt install ./EchoTransfer_*.deb`
- **.rpm:** `sudo dnf install ./EchoTransfer-*.rpm`

To remember your password between launches, Linux needs a keyring service such as GNOME Keyring or KWallet. Most desktops already have one.

## What you need

- A **Navidrome** server you can reach from your computer, with your username and password. Other Subsonic-compatible servers may work but aren't tested.
- The **SD card** from your Echo, in a card reader. It must be formatted as **FAT32** or **exFAT**, as the player expects.
- For **MP3 conversion** only: [ffmpeg](https://www.navidrome.org/docs/installation/pre-built-binaries/) installed on the Navidrome server. The official Navidrome Docker image already includes it.

## How to use it

1. **Connect to your server.** Enter your Navidrome address (for example `music.example.com` or `192.168.1.10:4533`), username and password, then click **Connect**. If you tick **Remember password**, the app connects by itself next time. Your password is stored in the system keychain (macOS Keychain, Windows Credential Manager, or the Linux keyring), not in a plain file.

2. **Insert the SD card.** It appears in the card menu at the top of the window within a couple of seconds. If only one card is inserted, it's chosen for you.

3. **Choose your music.**
   - Tick an artist in the left column to add all of their albums, or click the artist's name and tick single albums on the right.
   - Use the search box to find artists or albums.
   - Switch the list to **On card** to see only what you've selected.

4. **Check what will change.** Each album gets a label:

   | Label | Meaning |
   | --- | --- |
   | **New** | Not on the card yet. It will be copied. |
   | **Updated** | Changed on the server (new tags, new cover, replaced file) or missing from the card. It will be copied again. |
   | **Remove** | You deselected it, or it was deleted from the server. It will be removed from the card. |
   | **On card** | Already up to date. Nothing will happen. |

   Click an album to see the status of each track. The bar at the bottom shows how much space the card will have after syncing. If the selection doesn't fit, the **Sync** button stays disabled until you deselect something.

5. **Press Sync.** Progress, speed and time left are shown at the bottom. You can cancel at any time. Tracks that were already copied stay on the card, and the next sync continues from where it stopped.

6. **Eject the card** from your system before you take it out, then put it back in your Echo.

The next time you insert the same card, your selection is loaded from the card. Press **Sync** to bring it up to date with the server.

Added music to Navidrome while the app was open? Click the **reload** button next to the server name to fetch the library again.

## Settings

Open **Settings** with the gear button at the top right.

| Setting | What it does |
| --- | --- |
| **Audio format on card** | **Original files** copies tracks exactly as they are on the server (FLAC, MP3, WAV…). **MP3 320 / 256 / 192 kbps** asks Navidrome to convert each track, which saves a lot of space with FLAC libraries. This needs ffmpeg on the server. Changing it copies every track again. |
| **Embedded cover size** | The size of the cover image put into each track. **500 px** works well on the Echo. Try **300 px** if covers don't appear on your player. |
| **Parallel downloads** | How many tracks are downloaded at the same time (default 4). Raise it on a fast connection. Lower it if your server struggles. |
| **Folder layout** | Where tracks go on the card. See below. |
| **Delete macOS "._" files after syncing** | macOS sometimes leaves hidden `._` files on memory cards, which the player can list as broken tracks. Leave this on. |
| **Sign out** | Disconnects and removes the saved password. |

### Folder layout

The default layout is:

```
Music/{albumartist}/[{year} - ]{album}/[{disc}-]{track:02} {title}
```

which creates files like:

```
Music/Radiohead/1997 - OK Computer/02 Paranoid Android.flac
Music/Pink Floyd/1979 - The Wall/2-01 Hey You.flac      (multi-disc album)
```

- Available fields: `{albumartist}` `{artist}` `{album}` `{title}` `{year}` `{track}` `{disc}` `{genre}`.
- `{track:02}` pads the number with zeros (`2` becomes `02`).
- Text inside `[square brackets]` is left out when a field in it is empty. For example, an album without a year gets no `" - "` prefix. `{disc}` is only filled in for albums with more than one disc.
- The file extension is added automatically.
- Characters that memory cards don't allow (`< > : " / \ | ? *`) are replaced with `_`.

The settings window shows a live preview. If you change the layout later, existing files on the card are moved to their new place on the next sync. Nothing is downloaded again.

## How syncing works

EchoTransfer keeps a small file on the card, `.echotransfer/manifest.json`. It lists every track the app copied, where it put it, and what the track looked like on the server at that time. On each sync it compares that list with your selection and the server:

- **Not on the card** → copied.
- **Changed on the server** (file size, tags, cover, or format setting) → copied again.
- **Deleted from the card by hand** → copied again.
- **Only the folder layout changed** → moved on the card.
- **Deselected or gone from the server** → deleted, and empty folders are cleaned up.
- **Everything else** → left alone.

Each track is downloaded to a temporary folder on your computer first. The cover is embedded there, then the finished file is copied to the card. Until the copy is complete, the file has a `.part` name. If the card is pulled out mid-sync, you'll never end up with a half-written track that looks complete.

Because the list lives on the card, you can sync the same card from different computers. You can also use several cards with different selections.

**Your own files are safe.** Anything on the card that EchoTransfer didn't put there is never changed or deleted. If one of your files is already at the place where a track would go, the track gets a ` (2)` suffix instead.

## Troubleshooting

**"Can't reach the server"**
Check the address in a web browser on the same computer. Include the port if your server uses one (e.g. `192.168.1.10:4533`). If Navidrome runs under a sub-path, include it (e.g. `example.com/navidrome`). If you don't type `http://` or `https://`, the app uses `https://`, so type `http://` explicitly for a server without HTTPS.

**"Wrong username or password"**
Use the same login as the Navidrome web interface.

**My card doesn't show up in the menu**
Make sure the card is mounted: it should appear in Finder, File Explorer or your file manager. Wait a couple of seconds, since the list refreshes every 2 seconds.

**Covers don't show on the player**
- Make sure the album has a cover in Navidrome: it should be visible in the Navidrome web interface.
- In Settings, set **Embedded cover size** to **300 px** and sync again. Every track gets its cover re-embedded.

**"Server could not convert this track"**
MP3 conversion needs ffmpeg on the Navidrome server. Install it there, or switch **Audio format on card** back to **Original files**. The original files already on the card are kept when conversion fails.

**The player shows strange "._" tracks**
Turn on **Delete macOS "._" files after syncing** in Settings and sync again.

**Sync is blocked because the card is full**
The bar at the bottom shows by how much the selection is too big. Deselect some albums, or switch to MP3 conversion.

**I want to start over with a card**
Delete the `Music` folder and the hidden `.echotransfer` folder from the card, then sync again.

## Building from source

You need [Node.js](https://nodejs.org/) 20 or newer and [Rust](https://rustup.rs/), plus the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your system. On Linux, also install `libdbus-1-dev` and `pkg-config` for keyring support.

```sh
git clone https://github.com/TsvetomirGT/echotransfer.git
cd echotransfer
npm install
npm run tauri dev      # run the app in development mode
npm run tauri build    # build installers into src-tauri/target/release/bundle
```

Tests:

```sh
npm run check                 # type-check the interface
cd src-tauri && cargo test    # unit tests
```

To run the end-to-end test against a real Navidrome server, which syncs into a temporary folder:

```sh
cd src-tauri
ECHO_E2E_URL=http://127.0.0.1:4533 ECHO_E2E_USER=admin ECHO_E2E_PASS=secret \
  cargo test e2e -- --ignored --nocapture
```

### Project layout

| Path | Contents |
| --- | --- |
| `src/` | User interface (Svelte 5 + TypeScript) |
| `src-tauri/src/subsonic/` | Navidrome / Subsonic API client |
| `src-tauri/src/diff.rs` | Compares the selection with what's on the card |
| `src-tauri/src/sync/` | Downloading, cover embedding, writing to the card |
| `src-tauri/src/paths.rs` | Folder layout and file name rules |
| `src-tauri/src/manifest.rs` | The `.echotransfer/manifest.json` file on the card |

### Releasing

1. Set the new version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`.
2. Commit, then tag and push:
   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```
3. The **Release** workflow runs the tests, builds installers for macOS (Apple Silicon and Intel), Windows and Linux, and attaches them to a draft release. Review the draft on GitHub and publish it.

The workflow can also be started by hand from the **Actions** tab.
