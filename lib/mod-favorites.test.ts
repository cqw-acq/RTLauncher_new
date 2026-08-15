import { beforeEach, describe, expect, it, vi } from "vitest";
import { readModFavorites, writeModFavorites, type ModFavorite } from "./mod-favorites";

const favorite: ModFavorite = {
  id: "https://cdn.example.com/example.jar",
  slug: "example-mod",
  name: "Example Mod",
  versionLabel: "1.2.3",
  mcVersion: "1.20.1",
  modLoader: "fabric",
  downloadUrl: "https://cdn.example.com/example.jar",
  addedAt: 1,
};

describe("mod favorites", () => {
  beforeEach(() => {
    const values = new Map<string, string>();
    Object.defineProperty(window, "localStorage", {
      configurable: true,
      value: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value),
        removeItem: (key: string) => values.delete(key),
        clear: () => values.clear(),
      },
    });
  });

  it("persists valid favorites and announces the change", () => {
    const listener = vi.fn();
    window.addEventListener("rtlauncher:mod-favorites-changed", listener);

    writeModFavorites([favorite]);

    expect(readModFavorites()).toEqual([favorite]);
    expect(listener).toHaveBeenCalledOnce();
    window.removeEventListener("rtlauncher:mod-favorites-changed", listener);
  });

  it("ignores malformed persisted data", () => {
    window.localStorage.setItem("rtlauncher.mod-favorites.v1", JSON.stringify([{ id: "missing-fields" }]));
    expect(readModFavorites()).toEqual([]);
  });
});
