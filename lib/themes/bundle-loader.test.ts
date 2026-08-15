import { afterEach, describe, expect, it, vi } from "vitest";

import type { ThemeDefinition, ThemeManifest } from "./protocol";
import { loadThemeBundle } from "./bundle-loader";

function manifest(style = false): ThemeManifest {
  return {
    schemaVersion: "1.0",
    id: "com.example.nebula",
    name: "Nebula",
    version: "1.0.0",
    author: { name: "Example" },
    engines: { rtlauncher: ">=0.2.0", themeApi: "^1.0.0" },
    entry: {
      script: "dist/theme.js",
      style: style ? "dist/theme.css" : undefined,
    },
    supports: { colorSchemes: ["light", "dark"] },
  };
}

function definition(overrides: Partial<ThemeDefinition> = {}): ThemeDefinition {
  return {
    id: "com.example.nebula",
    version: "1.0.0",
    apiVersion: "1.0.0",
    setup() {},
    ...overrides,
  };
}

function loaderHarness(register?: () => void) {
  let sequence = 0;
  const revoked: string[] = [];
  const append = vi.spyOn(document.head, "appendChild").mockImplementation((node) => {
    if (node instanceof HTMLScriptElement) {
      queueMicrotask(() => {
        register?.();
        node.onload?.(new Event("load"));
      });
    }
    return node;
  });
  return {
    append,
    dependencies: {
      reader: {
        readText: vi.fn(async (_themeId: string, path: string) => {
          return path.endsWith(".css") ? ":root{--nebula:1}" : "bundle";
        }),
        readBinary: vi.fn(async () => btoa("asset")),
      },
      createObjectURL: vi.fn(() => `blob:theme-${++sequence}`),
      revokeObjectURL: vi.fn((url: string) => revoked.push(url)),
    },
    revoked,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
  delete window.__RTL_THEME_REGISTER__;
});

describe("loadThemeBundle", () => {
  it("accepts exactly one matching registration", async () => {
    const registered = definition();
    const harness = loaderHarness(() => window.__RTL_THEME_REGISTER__?.(registered));

    const loaded = await loadThemeBundle(manifest(), harness.dependencies);

    expect(loaded.definition).toBe(registered);
    expect(window.__RTL_THEME_REGISTER__).toBeUndefined();
    expect(harness.dependencies.reader.readText).toBeDefined();
    loaded.unload();
  });

  it("rejects an ID mismatch and duplicate registration", async () => {
    const mismatch = loaderHarness(() => {
      window.__RTL_THEME_REGISTER__?.(definition({ id: "com.example.other" }));
    });
    await expect(loadThemeBundle(manifest(), mismatch.dependencies)).rejects.toMatchObject({
      code: "THEME_BUNDLE_ID_MISMATCH",
    });

    const duplicate = loaderHarness(() => {
      window.__RTL_THEME_REGISTER__?.(definition());
      window.__RTL_THEME_REGISTER__?.(definition());
    });
    await expect(loadThemeBundle(manifest(), duplicate.dependencies)).rejects.toMatchObject({
      code: "THEME_BUNDLE_MULTIPLE_REGISTRATIONS",
    });
  });

  it("rejects a bundle that does not register a definition", async () => {
    const harness = loaderHarness();

    await expect(loadThemeBundle(manifest(), harness.dependencies)).rejects.toMatchObject({
      code: "THEME_BUNDLE_REGISTRATION_MISSING",
    });
  });

  it("activates CSS and removes all bundle resources on unload", async () => {
    const harness = loaderHarness(() => window.__RTL_THEME_REGISTER__?.(definition()));
    const loaded = await loadThemeBundle(manifest(true), harness.dependencies);

    expect(harness.append.mock.calls.some(([node]) => node instanceof HTMLLinkElement)).toBe(true);
    expect(harness.dependencies.createObjectURL).toHaveBeenCalledTimes(2);

    loaded.unload();

    expect(harness.revoked).toEqual(["blob:theme-1", "blob:theme-2"]);
  });

  it("creates and revokes package-scoped asset URLs", async () => {
    const harness = loaderHarness(() => window.__RTL_THEME_REGISTER__?.(definition()));
    const loaded = await loadThemeBundle(manifest(), harness.dependencies);

    const asset = await loaded.assets.url("assets/icon.png");
    loaded.assets.release(asset);

    expect(asset).toBe("blob:theme-2");
    expect(harness.dependencies.reader.readBinary).toHaveBeenCalledWith(
      "com.example.nebula",
      "assets/icon.png",
    );
    expect(harness.revoked).toContain("blob:theme-2");
    loaded.unload();
  });
});
