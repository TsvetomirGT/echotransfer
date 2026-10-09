import { api, type ArtistView, type DiffResponse, type ServerInfo, type Settings, type Volume } from "./api";

class AppState {
  server = $state<ServerInfo | null>(null);
  settings = $state<Settings | null>(null);
  artists = $state<ArtistView[]>([]);
  libraryLoading = $state(false);
  libraryError = $state<string | null>(null);
  volumes = $state<Volume[]>([]);
  mount = $state<string | null>(null);
  selection = $state<Set<string>>(new Set());
  diff = $state<DiffResponse | null>(null);
  diffError = $state<string | null>(null);
  diffLoading = $state(false);
  cardNotice = $state<string | null>(null);
  syncing = $state(false);

  get volume(): Volume | null {
    return this.volumes.find((v) => v.mountPoint === this.mount) ?? null;
  }

  coverUrl(id: string | null | undefined, size = 160): string | null {
    if (!id || !this.server) return null;
    return `${this.server.coverArtBase}&id=${encodeURIComponent(id)}&size=${size}`;
  }

  setVolumes(vs: Volume[]) {
    this.volumes = vs;
    if (this.mount && !vs.some((v) => v.mountPoint === this.mount)) {
      this.mount = null;
      this.diff = null;
    }
    if (!this.mount && vs.length === 1) void this.selectVolume(vs[0].mountPoint);
  }

  /** Switch card: load its saved selection from the manifest. */
  async selectVolume(mount: string | null) {
    this.mount = mount;
    this.diff = null;
    this.cardNotice = null;
    if (!mount) return;
    try {
      const info = await api.cardInfo(mount);
      this.selection = new Set(info.selection);
      if (info.hasManifest && !info.sameServer) {
        this.cardNotice = `This card was synced from ${info.server}. Syncing will replace those ${info.trackCount} tracks.`;
      }
    } catch (e) {
      this.cardNotice = String(e);
    }
    this.scheduleDiff();
  }

  toggleAlbums(ids: string[], on: boolean) {
    const next = new Set(this.selection);
    for (const id of ids) on ? next.add(id) : next.delete(id);
    this.selection = next;
    this.scheduleDiff();
  }

  private diffTimer: ReturnType<typeof setTimeout> | undefined;
  private diffSeq = 0;

  scheduleDiff(delay = 250) {
    clearTimeout(this.diffTimer);
    this.diffTimer = setTimeout(() => void this.refreshDiff(), delay);
  }

  async refreshDiff() {
    if (!this.mount || this.artists.length === 0) return;
    const seq = ++this.diffSeq;
    this.diffLoading = true;
    try {
      const d = await api.computeDiff(this.mount, [...this.selection]);
      if (seq === this.diffSeq) {
        this.diff = d;
        this.diffError = null;
      }
    } catch (e) {
      if (seq === this.diffSeq) this.diffError = String(e);
    } finally {
      if (seq === this.diffSeq) this.diffLoading = false;
    }
  }

  async loadLibrary(refresh = false) {
    this.libraryLoading = true;
    this.libraryError = null;
    try {
      this.artists = await api.loadLibrary(refresh);
      this.scheduleDiff(0);
    } catch (e) {
      this.libraryError = String(e);
    } finally {
      this.libraryLoading = false;
    }
  }

  async signOut() {
    await api.disconnect();
    this.server = null;
    this.artists = [];
    this.diff = null;
  }
}

export const app = new AppState();
