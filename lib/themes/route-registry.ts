import type {
  CoreRouteId,
  ThemePageRegistration,
  ThemeRouteRegistration,
  ThemeRouteRegistryAPI,
} from "./protocol";

export interface ThemeRouteRegistrySnapshot {
  revision: number;
}

interface OwnedRouteRegistration extends ThemeRouteRegistration {
  owner: string;
}

interface OwnedPageRegistration extends ThemePageRegistration {
  owner: string;
  normalizedPath: string;
}

function normalizePath(path: string): string {
  if (path.length > 1 && path.endsWith("/")) return path.slice(0, -1);
  return path;
}

export class ThemeRouteRegistry {
  private overrides: OwnedRouteRegistration[] = [];
  private pages: OwnedPageRegistration[] = [];
  private listeners = new Set<() => void>();
  private snapshot: ThemeRouteRegistrySnapshot = Object.freeze({ revision: 0 });

  forOwner(owner: string): ThemeRouteRegistryAPI {
    return {
      override: (registration) => this.registerOverride(owner, registration),
      add: (registration) => this.registerPage(owner, registration),
    };
  }

  registerOverride(owner: string, registration: ThemeRouteRegistration): () => void {
    this.assertOwner(owner);
    this.assertAvailableId(owner, registration.id);
    if (
      registration.mode === "replace" &&
      this.overrides.some((item) =>
        item.owner === owner && item.target === registration.target && item.mode === "replace"
      )
    ) {
      throw new Error(`A replacement for ${registration.target} is already registered.`);
    }

    const entry: OwnedRouteRegistration = Object.freeze({ ...registration, owner });
    this.overrides = [...this.overrides, entry];
    this.publish();
    return this.createDisposer(() => {
      const next = this.overrides.filter((item) => item !== entry);
      if (next.length === this.overrides.length) return false;
      this.overrides = next;
      return true;
    });
  }

  registerPage(owner: string, registration: ThemePageRegistration): () => void {
    this.assertOwner(owner);
    this.assertAvailableId(owner, registration.id);
    const normalizedPath = normalizePath(registration.path);
    if (
      this.pages.some((item) =>
        item.owner === owner && item.normalizedPath === normalizedPath
      )
    ) {
      throw new Error(`A page for ${normalizedPath} is already registered.`);
    }

    const entry: OwnedPageRegistration = Object.freeze({
      ...registration,
      owner,
      normalizedPath,
    });
    this.pages = [...this.pages, entry];
    this.publish();
    return this.createDisposer(() => {
      const next = this.pages.filter((item) => item !== entry);
      if (next.length === this.pages.length) return false;
      this.pages = next;
      return true;
    });
  }

  resolveCoreRoute(routeId: CoreRouteId, owner: string): {
    replacement?: ThemeRouteRegistration;
    wrappers: readonly ThemeRouteRegistration[];
  } {
    const matches = this.overrides.filter((item) =>
      item.owner === owner && item.target === routeId
    );
    return {
      replacement: matches.find((item) => item.mode === "replace"),
      wrappers: matches.filter((item) => item.mode === "wrap"),
    };
  }

  resolveVirtualRoute(
    path: string,
    owner: string,
  ): ThemePageRegistration | undefined {
    const normalizedPath = normalizePath(path);
    return this.pages.find((item) =>
      item.owner === owner && item.normalizedPath === normalizedPath
    );
  }

  clearOwner(owner: string): void {
    const nextOverrides = this.overrides.filter((item) => item.owner !== owner);
    const nextPages = this.pages.filter((item) => item.owner !== owner);
    if (
      nextOverrides.length === this.overrides.length &&
      nextPages.length === this.pages.length
    ) {
      return;
    }
    this.overrides = nextOverrides;
    this.pages = nextPages;
    this.publish();
  }

  getSnapshot(): ThemeRouteRegistrySnapshot {
    return this.snapshot;
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private assertOwner(owner: string): void {
    if (!owner.trim()) throw new Error("Theme owner is required.");
  }

  private assertAvailableId(owner: string, id: string): void {
    const exists = this.overrides.some((item) => item.owner === owner && item.id === id) ||
      this.pages.some((item) => item.owner === owner && item.id === id);
    if (exists) throw new Error(`Contribution ${id} is already registered.`);
  }

  private createDisposer(remove: () => boolean): () => void {
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      if (remove()) this.publish();
    };
  }

  private publish(): void {
    this.snapshot = Object.freeze({ revision: this.snapshot.revision + 1 });
    this.listeners.forEach((listener) => listener());
  }
}
