import type {
  CoreSlotId,
  ThemeSlotRegistration,
  ThemeSlotRegistryAPI,
} from "./protocol";

export interface ThemeSlotRegistrySnapshot {
  revision: number;
}

export interface ResolvedThemeSlot {
  replacement?: ThemeSlotRegistration;
  before: readonly ThemeSlotRegistration[];
  after: readonly ThemeSlotRegistration[];
  wrappers: readonly ThemeSlotRegistration[];
}

interface OwnedSlotRegistration extends ThemeSlotRegistration {
  owner: string;
  sequence: number;
}

function byOrder(left: OwnedSlotRegistration, right: OwnedSlotRegistration): number {
  return (left.order ?? 0) - (right.order ?? 0) || left.sequence - right.sequence;
}

export class ThemeSlotRegistry {
  private registrations: OwnedSlotRegistration[] = [];
  private listeners = new Set<() => void>();
  private snapshot: ThemeSlotRegistrySnapshot = Object.freeze({ revision: 0 });
  private nextSequence = 0;

  forOwner(owner: string): ThemeSlotRegistryAPI {
    return {
      register: (registration) => this.register(owner, registration),
    };
  }

  register<T>(owner: string, registration: ThemeSlotRegistration<T>): () => void {
    if (!owner.trim()) throw new Error("Theme owner is required.");
    if (
      this.registrations.some((item) =>
        item.owner === owner && item.id === registration.id
      )
    ) {
      throw new Error(`Contribution ${registration.id} is already registered.`);
    }
    if (
      registration.mode === "replace" &&
      this.registrations.some((item) =>
        item.owner === owner &&
        item.target === registration.target &&
        item.mode === "replace"
      )
    ) {
      throw new Error(`A replacement for ${registration.target} is already registered.`);
    }

    const entry = Object.freeze({
      ...registration,
      owner,
      sequence: this.nextSequence++,
    }) as OwnedSlotRegistration;
    this.registrations = [...this.registrations, entry];
    this.publish();

    let active = true;
    return () => {
      if (!active) return;
      active = false;
      const next = this.registrations.filter((item) => item !== entry);
      if (next.length === this.registrations.length) return;
      this.registrations = next;
      this.publish();
    };
  }

  resolve(slotId: CoreSlotId, owner: string): ResolvedThemeSlot {
    const matches = this.registrations
      .filter((item) => item.owner === owner && item.target === slotId)
      .sort(byOrder);
    return {
      replacement: matches.find((item) => item.mode === "replace"),
      before: matches.filter((item) => item.mode === "before"),
      after: matches.filter((item) => item.mode === "after"),
      wrappers: matches.filter((item) => item.mode === "wrap"),
    };
  }

  clearOwner(owner: string): void {
    const next = this.registrations.filter((item) => item.owner !== owner);
    if (next.length === this.registrations.length) return;
    this.registrations = next;
    this.publish();
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getSnapshot(): ThemeSlotRegistrySnapshot {
    return this.snapshot;
  }

  private publish(): void {
    this.snapshot = Object.freeze({ revision: this.snapshot.revision + 1 });
    this.listeners.forEach((listener) => listener());
  }
}
