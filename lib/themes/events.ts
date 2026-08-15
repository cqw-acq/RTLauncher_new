import type { JsonValue, ThemeEventService } from "./protocol";

interface EventListener {
  owner: string;
  listener(payload: JsonValue): void;
}

export class ThemeEventBus {
  private readonly listeners = new Map<string, Set<EventListener>>();

  forOwner(owner: string): ThemeEventService {
    if (!owner.trim()) throw new Error("Theme event owner is required.");
    return {
      emit: (name, payload) => this.emit(owner, name, payload),
      on: <T extends JsonValue>(name: string, listener: (payload: T) => void) =>
        this.on(owner, name, listener as (payload: JsonValue) => void),
    };
  }

  emit(owner: string, name: string, payload: JsonValue): void {
    if (!name.startsWith(`${owner}:`)) {
      throw new Error(`Theme events must use the ${owner}: namespace.`);
    }
    this.dispatch(name, payload);
  }

  emitCore(name: string, payload: JsonValue): void {
    if (!name.startsWith("core:")) {
      throw new Error("Core events must use the core: namespace.");
    }
    this.dispatch(name, payload);
  }

  clearOwner(owner: string): void {
    this.listeners.forEach((listeners, name) => {
      [...listeners]
        .filter((entry) => entry.owner === owner)
        .forEach((entry) => listeners.delete(entry));
      if (listeners.size === 0) this.listeners.delete(name);
    });
  }

  private on(
    owner: string,
    name: string,
    listener: (payload: JsonValue) => void,
  ): () => void {
    if (!name.trim()) throw new Error("Event name is required.");
    const entry = { owner, listener };
    const listeners = this.listeners.get(name) ?? new Set<EventListener>();
    listeners.add(entry);
    this.listeners.set(name, listeners);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      listeners.delete(entry);
      if (listeners.size === 0 && this.listeners.get(name) === listeners) {
        this.listeners.delete(name);
      }
    };
  }

  private dispatch(name: string, payload: JsonValue): void {
    const listeners = this.listeners.get(name);
    if (!listeners) return;
    [...listeners].forEach(({ owner, listener }) => {
      try {
        listener(payload);
      } catch (error) {
        console.error(`[Theme ${owner}] Event listener for ${name} failed.`, error);
      }
    });
  }
}
