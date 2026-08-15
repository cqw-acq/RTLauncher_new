export interface DevelopmentThemeWatcherOptions {
  debounceMs?: number;
  reload(themeId: string): void | Promise<void>;
}

export class DevelopmentThemeWatcher {
  private readonly debounceMs: number;
  private readonly timers = new Map<string, ReturnType<typeof setTimeout>>();
  private disposed = false;

  constructor(private readonly options: DevelopmentThemeWatcherOptions) {
    this.debounceMs = options.debounceMs ?? 250;
  }

  notifyChange(themeId: string): void {
    if (this.disposed) return;
    const current = this.timers.get(themeId);
    if (current !== undefined) clearTimeout(current);
    const timer = setTimeout(() => {
      this.timers.delete(themeId);
      if (!this.disposed) void this.options.reload(themeId);
    }, this.debounceMs);
    this.timers.set(themeId, timer);
  }

  dispose(): void {
    this.disposed = true;
    this.timers.forEach((timer) => clearTimeout(timer));
    this.timers.clear();
  }
}
