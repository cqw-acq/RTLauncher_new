import { afterEach, describe, expect, it, vi } from "vitest";

import { ThemeEventBus } from "./events";

afterEach(() => vi.restoreAllMocks());

describe("ThemeEventBus", () => {
  it("keeps new listeners when an old disposer runs after owner cleanup", () => {
    const bus = new ThemeEventBus();
    const oldOwner = bus.forOwner("com.example.old");
    const newOwner = bus.forOwner("com.example.new");
    const disposeOld = oldOwner.on("core:changed", () => undefined);
    bus.clearOwner("com.example.old");
    const received: string[] = [];
    newOwner.on("core:changed", (payload) => received.push(String(payload)));

    disposeOld();
    bus.emitCore("core:changed", "ready");

    expect(received).toEqual(["ready"]);
  });

  it("continues dispatch after a listener fails", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const bus = new ThemeEventBus();
    const first = bus.forOwner("com.example.first");
    const second = bus.forOwner("com.example.second");
    const received: string[] = [];
    first.on("core:changed", () => { throw new Error("broken listener"); });
    second.on("core:changed", (payload) => received.push(String(payload)));

    expect(() => bus.emitCore("core:changed", "ready")).not.toThrow();
    expect(received).toEqual(["ready"]);
  });
});
