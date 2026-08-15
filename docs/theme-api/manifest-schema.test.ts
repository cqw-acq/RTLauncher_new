// @vitest-environment node

import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import Ajv2020 from "ajv/dist/2020";
import { describe, expect, it } from "vitest";

async function validator() {
  const schema = JSON.parse(await readFile(
    resolve("docs/theme-api/manifest.schema.json"),
    "utf8",
  ));
  return new Ajv2020({ allErrors: true }).compile(schema);
}

const validManifest = {
  schemaVersion: "1.0",
  id: "com.example.nebula",
  name: "Nebula",
  version: "1.2.0",
  author: { name: "Example" },
  engines: { rtlauncher: ">=1.0.0 <2.0.0", themeApi: "^1.0.0" },
  entry: { script: "dist/theme.js", style: "dist/theme.css" },
  supports: { colorSchemes: ["light", "dark"] },
};

describe("Theme manifest JSON Schema", () => {
  it("accepts development and packaged manifests", async () => {
    const validate = await validator();
    expect(validate(validManifest), JSON.stringify(validate.errors)).toBe(true);
    expect(validate({
      ...validManifest,
      integrity: {
        algorithm: "sha256",
        files: { "dist/theme.js": `sha256-${"a".repeat(64)}` },
      },
    }), JSON.stringify(validate.errors)).toBe(true);
  });

  it("accepts an explicit unsafe command request", async () => {
    const validate = await validator();

    expect(validate({
      ...validManifest,
      permissions: { unsafeCommands: ["get_system_info"] },
    }), JSON.stringify(validate.errors)).toBe(true);
  });

  it.each([
    ["invalid ID", { ...validManifest, id: "Nebula" }],
    ["unsafe entry", { ...validManifest, entry: { script: "../theme.js" } }],
    ["single-dot package path", { ...validManifest, icon: "." }],
    ["extension key without a namespace", {
      ...validManifest,
      extensions: { updateChannel: "stable" },
    }],
    ["route outside the Theme namespace", {
      ...validManifest,
      contributes: {
        routes: [{ id: "example.page", path: "/outside", mode: "replace" }],
      },
    }],
    ["invalid color scheme", {
      ...validManifest,
      supports: { colorSchemes: ["automatic"] },
    }],
    ["missing author", { ...validManifest, author: undefined }],
  ])("rejects %s", async (_name, fixture) => {
    const validate = await validator();
    expect(validate(fixture)).toBe(false);
  });
});
