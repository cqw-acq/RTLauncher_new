import { describe, expect, it } from "vitest";

import {
  BUILTIN_THEME_ID,
  CORE_ROUTE_IDS,
  CORE_SLOT_IDS,
  THEME_API_VERSION,
  THEME_SCHEMA_VERSION,
  isCoreRouteId,
  isCoreSlotId,
} from "./protocol";

describe("Theme protocol", () => {
  it("publishes stable protocol identifiers", () => {
    expect(THEME_SCHEMA_VERSION).toBe("1.0");
    expect(THEME_API_VERSION).toBe("1.0.0");
    expect(BUILTIN_THEME_ID).toBe("builtin.default");
  });

  it("accepts only published core route identifiers", () => {
    expect(CORE_ROUTE_IDS).toEqual([
      "core.home",
      "core.launch",
      "core.download",
      "core.download.detail",
      "core.multiplayer",
      "core.tools",
      "core.settings",
      "core.game-settings",
      "core.instance.mods",
      "core.instance.worlds",
      "core.instance.resources",
      "core.instance.shaders",
      "core.instance.screenshots",
      "core.instance.schematics",
    ]);
    expect(isCoreRouteId("core.home")).toBe(true);
    expect(isCoreRouteId("core.home.extra")).toBe(false);
  });

  it("accepts only published core slot identifiers", () => {
    expect(CORE_SLOT_IDS).toEqual([
      "app.titlebar.leading",
      "app.titlebar.center",
      "app.titlebar.actions",
      "app.sidebar",
      "app.sidebar.header",
      "app.sidebar.navigation",
      "app.sidebar.footer",
      "app.content.before",
      "app.content.after",
      "page.header",
      "page.header.actions",
      "page.footer",
      "launch.primary-action",
    ]);
    expect(isCoreSlotId("app.sidebar")).toBe(true);
    expect(isCoreSlotId("app.sidebar.unknown")).toBe(false);
  });
});
