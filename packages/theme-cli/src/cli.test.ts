// @vitest-environment node

import { createHash } from "node:crypto";
import { mkdtemp, readFile, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import {
  buildTheme,
  inspectThemeArchive,
  packTheme,
  scopeThemeCss,
  validateThemeManifest,
} from "./cli";

const temporaryDirectories: string[] = [];

async function fixture(): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "rtl-theme-cli-"));
  temporaryDirectories.push(root);
  await mkdir(join(root, "src"));
  await writeFile(join(root, "manifest.json"), JSON.stringify({
    schemaVersion: "1.0",
    id: "com.example.hello",
    name: "Hello",
    version: "1.0.0",
    author: { name: "Example" },
    engines: { rtlauncher: ">=1.0.0", themeApi: "^1.0.0" },
    entry: { script: "src/theme.tsx", style: "src/theme.css" },
    supports: { colorSchemes: ["light", "dark"] },
  }, null, 2));
  await writeFile(join(root, "src/theme.tsx"), `
    import React from "react";
    import { Button } from "@rtlauncher/theme-ui";
    import { defineTheme } from "@rtlauncher/theme-sdk";
    export default defineTheme({
      id: "com.example.hello", version: "1.0.0", apiVersion: "1.0.0",
      setup(context) {
        context.slots.register({ id: "hello", target: "page.header.actions", mode: "after", component: () => React.createElement(Button, null, "Hello") });
      }
    });
  `);
  await writeFile(join(root, "src/theme.css"), ".hello { color: red; }");
  return root;
}

afterEach(async () => {
  const { rm } = await import("node:fs/promises");
  await Promise.all(temporaryDirectories.splice(0).map((path) => rm(path, { recursive: true })));
});

describe("Theme author CLI", () => {
  it("validates IDs and safe package paths", () => {
    expect(validateThemeManifest({
      schemaVersion: "1.0",
      id: "Bad ID",
      entry: { script: "../theme.js" },
    })).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: "THEME_ID_INVALID" }),
      expect.objectContaining({ code: "THEME_PATH_INVALID" }),
    ]));
  });

  it("externalizes React and host UI through the runtime bridge", async () => {
    const root = await fixture();
    const result = await buildTheme(root);
    const script = await readFile(result.scriptPath, "utf8");

    expect(script).toContain("__RTL_THEME_HOST__");
    expect(script).toContain("__RTL_THEME_REGISTER__");
    expect(script).not.toContain('require("react")');
  });

  it("writes SHA-256 integrity data and scoped CSS", async () => {
    const root = await fixture();
    const result = await buildTheme(root);
    const script = await readFile(result.scriptPath);
    const manifest = JSON.parse(await readFile(result.manifestPath, "utf8"));
    const digest = createHash("sha256").update(script).digest("hex");

    expect(manifest.integrity.files["dist/theme.js"]).toBe(`sha256-${digest}`);
    expect(await readFile(result.stylePath!, "utf8")).toContain(
      '@scope ([data-rtl-theme="com.example.hello"])',
    );
    expect(scopeThemeCss("com.example.hello", ".x{}"))
      .toContain('[data-rtl-theme="com.example.hello"]');
  });

  it("creates deterministic archives that can be inspected", async () => {
    const root = await fixture();
    await buildTheme(root);
    const first = await packTheme(root, join(root, "first.rtltheme"));
    const second = await packTheme(root, join(root, "second.rtltheme"));

    expect(await readFile(first)).toEqual(await readFile(second));
    const inspection = await inspectThemeArchive(first);
    expect(inspection.manifest.id).toBe("com.example.hello");
    expect(inspection.files).toEqual([
      "dist/theme.css",
      "dist/theme.js",
      "manifest.json",
    ]);
    expect(inspection.integrityValid).toBe(true);
  });
});
