import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Status = "new" | "updated" | "removed" | "synced";
export type Profile = "original" | "mp3-320" | "mp3-256" | "mp3-192";

export interface ServerInfo {
  url: string;
  username: string;
  server: string;
  coverArtBase: string;
}

export interface Settings {
  serverUrl: string;
  username: string;
  profile: Profile;
  coverSize: number;
  concurrency: number;
  pathTemplate: string;
  cleanAppleDouble: boolean;
}

export interface Volume {
  name: string;
  mountPoint: string;
  fileSystem: string;
  totalBytes: number;
  availableBytes: number;
  removable: boolean;
}

export interface AlbumView {
  id: string;
  name: string;
  artist: string;
  year: number | null;
  coverArt: string | null;
  songCount: number;
  size: number;
  duration: number;
}

export interface ArtistView {
  id: string;
  name: string;
  albums: AlbumView[];
}

export interface CardInfo {
  hasManifest: boolean;
  selection: string[];
  trackCount: number;
  server: string;
  sameServer: boolean;
}

export interface Summary {
  new: number;
  updated: number;
  removed: number;
  synced: number;
  downloadBytes: number;
  freedBytes: number;
}

export interface AlbumDiff {
  status: Status | null;
  new: number;
  updated: number;
  removed: number;
  synced: number;
}

export interface DiffResponse {
  summary: Summary;
  albums: Record<string, AlbumDiff>;
  availableBytes: number;
  totalBytes: number;
  projectedFree: number;
}

export interface TrackRow {
  id: string;
  title: string;
  artist: string;
  track: number | null;
  disc: number | null;
  duration: number;
  size: number;
  suffix: string;
  status: Status | null;
}

export interface Progress {
  phase: string;
  done: number;
  total: number;
  bytesDone: number;
  bytesTotal: number;
  current: string | null;
  currentCover: string | null;
  failed: number;
}

export interface Outcome {
  downloaded: number;
  moved: number;
  deleted: number;
  bytes: number;
  failures: { title: string; relPath: string; error: string }[];
  warnings: string[];
  cancelled: boolean;
}

export interface SyncDone {
  outcome: Outcome | null;
  error: string | null;
}

export const api = {
  connect: (url: string, username: string, password: string, remember: boolean) =>
    invoke<ServerInfo>("connect", { url, username, password, remember }),
  autoConnect: () => invoke<ServerInfo | null>("auto_connect"),
  disconnect: () => invoke<void>("disconnect"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  listVolumes: () => invoke<Volume[]>("list_volumes"),
  loadLibrary: (refresh = false) => invoke<ArtistView[]>("load_library", { refresh }),
  cardInfo: (mount: string) => invoke<CardInfo>("card_info", { mount }),
  computeDiff: (mount: string, selection: string[]) =>
    invoke<DiffResponse>("compute_diff", { mount, selection }),
  albumTracks: (albumId: string) => invoke<TrackRow[]>("album_tracks", { albumId }),
  startSync: (mount: string, selection: string[]) => invoke<void>("start_sync", { mount, selection }),
  cancelSync: () => invoke<void>("cancel_sync"),
  onVolumes: (cb: (v: Volume[]) => void): Promise<UnlistenFn> =>
    listen<Volume[]>("volumes-changed", (e) => cb(e.payload)),
  onProgress: (cb: (p: Progress) => void): Promise<UnlistenFn> =>
    listen<Progress>("sync-progress", (e) => cb(e.payload)),
  onDone: (cb: (d: SyncDone) => void): Promise<UnlistenFn> =>
    listen<SyncDone>("sync-done", (e) => cb(e.payload)),
};

export function formatBytes(n: number): string {
  const abs = Math.abs(n);
  if (abs < 1000) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = abs;
  let i = -1;
  do {
    v /= 1000;
    i++;
  } while (v >= 1000 && i < units.length - 1);
  const s = v >= 100 ? v.toFixed(0) : v.toFixed(1);
  return `${n < 0 ? "−" : ""}${s} ${units[i]}`;
}

export function formatDuration(sec: number): string {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  if (h > 0) return `${h} h ${m} min`;
  return `${m}:${String(s).padStart(2, "0")}`;
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
