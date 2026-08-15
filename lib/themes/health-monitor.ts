export interface ThemeHealthMonitorOptions {
  threshold?: number;
  windowMs?: number;
  healthyDelayMs?: number;
  now?(): number;
  onHealthy?(themeId: string): void | Promise<void>;
  onRollback(themeId: string): void | Promise<void>;
}

export class ThemeHealthMonitor {
  private readonly threshold: number;
  private readonly windowMs: number;
  private readonly healthyDelayMs: number;
  private activeThemeId: string | undefined;
  private errors: number[] = [];
  private healthyTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(private readonly options: ThemeHealthMonitorOptions) {
    this.threshold = options.threshold ?? 3;
    this.windowMs = options.windowMs ?? 30_000;
    this.healthyDelayMs = options.healthyDelayMs ?? 5_000;
  }

  start(themeId: string): void {
    this.stop();
    this.activeThemeId = themeId;
    this.errors = [];
    this.healthyTimer = setTimeout(() => {
      this.healthyTimer = undefined;
      if (this.activeThemeId === themeId) void this.options.onHealthy?.(themeId);
    }, this.healthyDelayMs);
  }

  reportError(themeId: string): void {
    if (themeId !== this.activeThemeId) return;
    const now = (this.options.now ?? Date.now)();
    this.errors = this.errors.filter((timestamp) => now - timestamp <= this.windowMs);
    this.errors.push(now);
    if (this.errors.length < this.threshold) return;
    this.cancelHealthyTimer();
    this.activeThemeId = undefined;
    this.errors = [];
    void this.options.onRollback(themeId);
  }

  stop(): void {
    this.cancelHealthyTimer();
    this.activeThemeId = undefined;
    this.errors = [];
  }

  dispose(): void {
    this.stop();
  }

  private cancelHealthyTimer(): void {
    if (this.healthyTimer !== undefined) clearTimeout(this.healthyTimer);
    this.healthyTimer = undefined;
  }
}
