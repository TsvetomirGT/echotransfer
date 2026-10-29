import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { api, errorText } from "./api";

const RELEASES_URL = "https://github.com/TsvetomirGT/echotransfer/releases/latest";

type Status = "idle" | "checking" | "uptodate" | "available" | "downloading" | "error";

class Updater {
  current = $state("");
  available = $state<Update | null>(null);
  status = $state<Status>("idle");
  error = $state<string | null>(null);
  downloaded = $state(0);
  total = $state(0);
  dismissed = $state(false);
  /** False for Linux .deb/.rpm installs: those get a link to the release page. */
  canSelfUpdate = $state(true);

  /** Silent checks (at startup) don't report errors, e.g. when offline. */
  async check(silent = false) {
    if (this.status === "checking" || this.status === "downloading") return;
    this.status = "checking";
    this.error = null;
    try {
      if (!this.current) {
        [this.current, this.canSelfUpdate] = await Promise.all([getVersion(), api.canSelfUpdate()]);
      }
      this.available = await check();
      this.status = this.available ? "available" : "uptodate";
      if (this.available) this.dismissed = false;
    } catch (e) {
      this.status = silent ? "idle" : "error";
      this.error = silent ? null : errorText(e);
    }
  }

  /** Download, install and restart into the new version. */
  async install() {
    const update = this.available;
    if (!update) return;
    if (!this.canSelfUpdate) {
      await openUrl(RELEASES_URL);
      return;
    }
    this.status = "downloading";
    this.error = null;
    this.downloaded = 0;
    this.total = 0;
    try {
      await update.downloadAndInstall((e) => {
        if (e.event === "Started") this.total = e.data.contentLength ?? 0;
        else if (e.event === "Progress") this.downloaded += e.data.chunkLength;
      });
      await relaunch();
    } catch (e) {
      this.status = "error";
      this.error = errorText(e);
    }
  }
}

export const updater = new Updater();
