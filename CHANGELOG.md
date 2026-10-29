# Changelog

## 0.2.1

### New

- **`{group}` folder layout field.** Puts a track's grouping tag in the folder layout, for example `Music/{albumartist}/[{group}/]{album}/…`. The new Settings option **Use TIT1 / Work tag as {group}** covers libraries that keep their grouping in `TIT1`, which Navidrome reads as the *work*.

## 0.2.0

### New

- **Format menu in the toolbar.** Switch between **Original** and **MP3** right next to the SD card picker. Original stays the default. Your Navidrome server converts the tracks, so it needs ffmpeg installed.
- **MP3 quality setting.** Pick 320, 256 or 192 kbps in Settings. It's used whenever the format menu is set to MP3.
- **Automatic updates.** EchoTransfer checks for a new version at every launch. When one is out, a banner offers a one-click **Update** that downloads it, installs it and restarts the app. **Settings → Check for updates** shows your version and checks by hand.

### Fixed

- Choosing MP3 conversion in Settings failed with an error, so the card always got the original files. MP3 conversion works now.

### Notes

- **Updating from 0.1.x:** 0.1.x can't update itself, so install 0.2.0 by hand once. Later versions install from inside the app.
- **Linux:** only the AppImage can update itself. With the `.deb` or `.rpm`, the Update button opens the download page instead.
- **Switching the format:** going from Original to MP3, or changing the MP3 quality, copies every track to the card again on the next sync.
