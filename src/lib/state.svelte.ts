import { api, type AlbumView, type ArtistView, type DiffResponse, type Profile, type ServerInfo, type Settings, type Volume } from "./api";

export type AlbumFilter = "all" | "missing" | "recent" | "updated" | "onCard";

export const FILTERS: { id: AlbumFilter; label: string; title: string }[] = [
  { id: "all", label: "All", title: "Every album on the server" },
  { id: "missing", label: "Not on card", title: "Albums not selected for this card" },
  { id: "recent", label: "Recently added", title: "Added to the server in the last 30 days" },
  { id: "updated", label: "Updated", title: "On the card, but changed on the server" },
  { id: "onCard", label: "On card", title: "Albums selected for this card" },
];

const RECENT_MS = 30 * 24 * 60 * 60 * 1000;

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
  query = $state("");
  filter = $state<AlbumFilter>("all");

  /** Artists with at least one album passing the query and filter, with those albums. */
  visible = $derived.by(() =>
    this.artists
      .map((artist) => ({ artist, albums: this.queried(artist).filter((al) => this.passes(al, this.filter)) }))
      .filter((v) => v.albums.length > 0),
  );

  /** Every album the sidebar currently shows, for "add all". */
  matching = $derived(this.visible.flatMap((v) => v.albums));

  /** Album count per filter under the current query. */
  filterCounts = $derived.by(() => {
    const counts = Object.fromEntries(FILTERS.map((f) => [f.id, 0])) as Record<AlbumFilter, number>;
    for (const a of this.artists) {
      for (const al of this.queried(a)) {
        for (const f of FILTERS) if (this.passes(al, f.id)) counts[f.id]++;
      }
    }
    return counts;
  });

  /** Albums of `a` matching the search; an artist-name match keeps all of them. */
  private queried(a: ArtistView): AlbumView[] {
    const q = this.query.trim().toLowerCase();
    if (!q || a.name.toLowerCase().includes(q)) return a.albums;
    return a.albums.filter((al) => al.name.toLowerCase().includes(q));
  }

  private passes(al: AlbumView, f: AlbumFilter): boolean {
    switch (f) {
      case "all":
        return true;
      case "missing":
        return !this.selection.has(al.id);
      case "onCard":
        return this.selection.has(al.id);
      case "updated":
        return this.diff?.albums[al.id]?.status === "updated";
      case "recent":
        return !!al.created && Date.now() - Date.parse(al.created) < RECENT_MS;
    }
  }

  /** Albums of `a` the sidebar shows, for the album pane. */
  visibleAlbums(a: ArtistView): AlbumView[] {
    return this.visible.find((v) => v.artist.id === a.id)?.albums ?? [];
  }

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

  /** Toolbar format switch: original files or MP3 at the bitrate from settings. */
  async setFormat(format: "original" | "mp3") {
    if (!this.settings) return;
    const profile: Profile = format === "original" ? "original" : (`mp3-${this.settings.mp3Bitrate}` as Profile);
    this.settings = await api.saveSettings({ ...this.settings, profile });
    this.scheduleDiff(0);
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
