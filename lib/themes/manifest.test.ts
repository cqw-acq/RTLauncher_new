import { describe, expect, it } from "vitest";

import {
  isSafeThemePath,
  validateThemeManifest,
  type ThemeManifestValidationHost,
} from "./manifest";

const host: ThemeManifestValidationHost = {
  appVersion: "0.2.0",
  themeApiVersion: "1.0.0",
  schemaVersion: "1.0",
};

function validManifest(): Record<string, unknown> {
  return {
    schemaVersion: "1.0",
    id: "com.example.nebula",
    name: "Nebula",
    version: "1.2.0",
    author: { name: "Example Studio", url: "https://example.com" },
    engines: {
      rtlauncher: ">=0.2.0 <1.0.0",
      themeApi: "^1.0.0",
    },
    entry: {
      script: "dist/theme.js",
      style: "dist/theme.css",
    },
    supports: {
      colorSchemes: ["light", "dark"],
      locales: ["zh-CN", "en-US"],
      userOverrides: ["fontSize"],
    },
    contributes: {
      routes: [
        {
          id: "nebula.home",
          target: "core.home",
          mode: "replace",
        },
      ],
      slots: [
        {
          id: "nebula.sidebar",
          target: "app.sidebar",
          mode: "replace",
        },
      ],
      settings: {
        schema: "settings.schema.json",
        defaults: "settings.defaults.json",
      },
    },
    disclosures: ["instances.read", "unsafe.tauri.invoke"],
    integrity: {
      algorithm: "sha256",
      files: {
        "dist/theme.js": "sha256-1234",
        "dist/theme.css": "sha256-5678",
      },
    },
    extensions: {
      "com.example.nebula:updateChannel": "stable",
    },
  };
}

describe("isSafeThemePath", () => {
  it.each([
    "dist/theme.js",
    "assets/icons/logo.png",
    "settings.schema.json",
  ])("accepts a package-relative path: %s", (path) => {
    expect(isSafeThemePath(path)).toBe(true);
  });

  it.each([
    "",
    "/dist/theme.js",
    "C:/theme/theme.js",
    "../theme.js",
    "dist/../theme.js",
    "dist\\theme.js",
    "dist//theme.js",
  ])("rejects an unsafe package path: %s", (path) => {
    expect(isSafeThemePath(path)).toBe(false);
  });
});

describe("validateThemeManifest", () => {
  it("returns the normalized manifest when it is compatible", () => {
    const result = validateThemeManifest(validManifest(), host);

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.manifest.id).toBe("com.example.nebula");
      expect(result.manifest.engines.themeApi).toBe("^1.0.0");
      expect(result.warnings).toEqual([]);
    }
  });

  it.each([
    ["schemaVersion", "2.0", "THEME_SCHEMA_INCOMPATIBLE"],
    ["themeApi", "^2.0.0", "THEME_API_INCOMPATIBLE"],
    ["rtlauncher", ">=1.0.0", "THEME_APP_INCOMPATIBLE"],
  ])("rejects an incompatible %s", (field, value, code) => {
    const manifest = validManifest();
    if (field === "schemaVersion") {
      manifest.schemaVersion = value;
    } else {
      (manifest.engines as Record<string, unknown>)[field] = value;
    }

    const result = validateThemeManifest(manifest, host);

    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.issues.map((issue) => issue.code)).toContain(code);
  });

  it.each(["Nebula", "com..nebula", "com.example.nebula!", "builtin.default"])(
    "rejects the invalid or reserved ID %s",
    (id) => {
      const manifest = validManifest();
      manifest.id = id;

      const result = validateThemeManifest(manifest, host);

      expect(result.ok).toBe(false);
      if (!result.ok) {
        expect(result.issues).toContainEqual(
          expect.objectContaining({ code: "THEME_ID_INVALID", path: "id" }),
        );
      }
    },
  );

  it("rejects unsafe entry and asset paths", () => {
    const manifest = validManifest();
    (manifest.entry as Record<string, unknown>).script = "../theme.js";
    manifest.icon = "/tmp/icon.png";

    const result = validateThemeManifest(manifest, host);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.issues.map((issue) => issue.path)).toEqual(
        expect.arrayContaining(["entry.script", "icon"]),
      );
    }
  });

  it("rejects duplicate contribution IDs", () => {
    const manifest = validManifest();
    const contributes = manifest.contributes as Record<string, unknown>;
    contributes.slots = [
      {
        id: "nebula.home",
        target: "app.sidebar",
        mode: "after",
      },
    ];

    const result = validateThemeManifest(manifest, host);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.issues).toContainEqual(
        expect.objectContaining({ code: "THEME_CONTRIBUTION_DUPLICATE" }),
      );
    }
  });
});
