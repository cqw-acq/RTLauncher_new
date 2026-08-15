export class ThemeSettingsSubscriptions {
  private readonly listeners = new Set<() => void>();

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  notify(): void {
    [...this.listeners].forEach((listener) => listener());
  }
}
