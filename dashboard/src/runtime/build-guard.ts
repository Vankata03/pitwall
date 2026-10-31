const STORAGE_KEY = 'pitwall.reloadedForBuild';

export interface BuildGuardOptions {
  /** Build ID baked into this page. */
  ownBuildId: string;
  reload: () => void;
  storage: Pick<Storage, 'getItem' | 'setItem'>;
}

/**
 * Reloads the page when the collector's build ID differs from this page's. In edit mode it
 * raises a prompt instead, so unsaved edits survive; leaving edit mode (Done or Cancel)
 * leaves nothing unsaved, so a pending prompt then reloads. Each collector build triggers
 * at most one automatic reload, so a stale cache or a dev server cannot loop.
 */
export class BuildGuard {
  private editing = false;
  private pending: string | null = null;
  private prompt = false;
  private readonly listeners = new Set<() => void>();

  constructor(private readonly options: BuildGuardOptions) {}

  subscribe = (l: () => void): (() => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };
  /** True while the reload prompt should show. */
  getSnapshot = (): boolean => this.prompt;

  onHello(collectorBuildId: string): void {
    if (collectorBuildId === this.options.ownBuildId) return;
    if (this.options.storage.getItem(STORAGE_KEY) === collectorBuildId) return;
    this.pending = collectorBuildId;
    if (this.editing) this.setPrompt(true);
    else this.reloadNow();
  }

  setEditing(editing: boolean): void {
    this.editing = editing;
    if (!editing && this.pending !== null) this.reloadNow();
  }

  reloadNow(): void {
    if (this.pending !== null) this.options.storage.setItem(STORAGE_KEY, this.pending);
    this.pending = null;
    this.setPrompt(false);
    this.options.reload();
  }

  private setPrompt(v: boolean): void {
    if (this.prompt === v) return;
    this.prompt = v;
    for (const l of [...this.listeners]) l();
  }
}
