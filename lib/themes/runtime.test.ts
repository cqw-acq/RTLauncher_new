import type { ComponentType } from "react";
import { describe, expect, it, vi } from "vitest";

import type {
  JsonValue,
  RTLauncherThemeSDK,
  ThemeContext,
  ThemeDefinition,
  ThemeManifest,
  ThemeRouteComponentProps,
} from "./protocol";
import { BUILTIN_THEME_ID } from "./protocol";
import { ThemeRouteRegistry } from "./route-registry";
import { ThemeRuntime, ThemeRuntimeError } from "./runtime";
import { ThemeSlotRegistry } from "./slot-registry";

const EmptyPage: ComponentType<ThemeRouteComponentProps> = () => null;

function manifest(id: string, version = "1.0.0"): ThemeManifest {
  return {
    schemaVersion: "1.0",
    id,
    name: id,
    version,
    author: { name: "Test" },
    engines: { rtlauncher: ">=0.2.0", themeApi: "^1.0.0" },
    entry: { script: "dist/theme.js" },
    supports: { colorSchemes: ["light", "dark"] },
  };
}

function createContextServices(): Omit<
  ThemeContext,
  "manifest" | "runtime" | "routes" | "slots"
> {
  return {
    sdk: {} as RTLauncherThemeSDK,
    assets: {
      async url(path) { return `theme://${path}`; },
      release() {},
    },
    settings: {
      async get<T extends JsonValue>() { return {} as T; },
      async update<T extends JsonValue>(value: T) { return value; },
      async reset() {},
      subscribe() { return () => undefined; },
      registerMigration() { return () => undefined; },
    },
    events: {
      emit() {},
      on() { return () => undefined; },
    },
    logger: {
      debug() {},
      info() {},
      warn() {},
      error() {},
    },
  };
}

function createRuntime(options: { setupTimeoutMs?: number; activateTimeoutMs?: number } = {}) {
  const routes = new ThemeRouteRegistry();
  const slots = new ThemeSlotRegistry();
  const runtime = new ThemeRuntime({
    appVersion: "0.2.0",
    platform: "linux",
    routes,
    slots,
    createContextServices,
    ...options,
  });
  return { runtime, routes, slots };
}

describe("ThemeRuntime", () => {
  it("prepares registrations and activates a theme", async () => {
    const { runtime, routes } = createRuntime();
    const events: string[] = [];
    const definition: ThemeDefinition = {
      id: "com.example.nebula",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup(context) {
        events.push("setup");
        context.routes.override({
          id: "nebula.home",
          target: "core.home",
          mode: "replace",
          component: EmptyPage,
        });
        return {
          activate() { events.push("activate"); },
        };
      },
    };

    await runtime.prepareTheme(manifest(definition.id), definition);
    expect(runtime.getSnapshot().activeThemeId).toBe(BUILTIN_THEME_ID);

    await runtime.activateTheme(definition.id);

    const snapshot = runtime.getSnapshot();
    expect(events).toEqual(["setup", "activate"]);
    expect(snapshot.activeThemeId).toBe(definition.id);
    expect(routes.resolveCoreRoute("core.home", snapshot.activeOwner).replacement?.id).toBe(
      "nebula.home",
    );
  });

  it("cleans registrations when setup fails", async () => {
    const { runtime, routes } = createRuntime();
    const definition: ThemeDefinition = {
      id: "com.example.broken",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup(context) {
        context.routes.override({
          id: "broken.home",
          target: "core.home",
          mode: "replace",
          component: EmptyPage,
        });
        throw new Error("setup failed");
      },
    };

    await expect(runtime.prepareTheme(manifest(definition.id), definition)).rejects.toMatchObject({
      code: "THEME_SETUP_FAILED",
    });

    expect(routes.getSnapshot().revision).toBe(2);
    expect(runtime.getSnapshot().preparedThemeIds).toEqual([]);
  });

  it("keeps the active theme when the next activation fails", async () => {
    const { runtime } = createRuntime();
    const events: string[] = [];
    await runtime.prepareTheme(manifest("com.example.stable"), {
      id: "com.example.stable",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup() {
        return { deactivate() { events.push("stable.deactivate"); } };
      },
    });
    await runtime.activateTheme("com.example.stable");
    await runtime.prepareTheme(manifest("com.example.broken"), {
      id: "com.example.broken",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup() {
        return { activate() { throw new Error("activation failed"); } };
      },
    });

    await expect(runtime.activateTheme("com.example.broken")).rejects.toMatchObject({
      code: "THEME_ACTIVATION_FAILED",
    });

    expect(runtime.getSnapshot().activeThemeId).toBe("com.example.stable");
    expect(events).toEqual([]);
  });

  it("aborts activation after its timeout", async () => {
    vi.useFakeTimers();
    const { runtime } = createRuntime({ activateTimeoutMs: 50 });
    let signal: AbortSignal | undefined;
    await runtime.prepareTheme(manifest("com.example.slow"), {
      id: "com.example.slow",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup() {
        return {
          activate(event) {
            signal = event.signal;
            return new Promise<void>(() => undefined);
          },
        };
      },
    });

    const activation = runtime.activateTheme("com.example.slow");
    const rejection = expect(activation).rejects.toEqual(
      expect.objectContaining({ code: "THEME_ACTIVATION_TIMEOUT" }),
    );
    await vi.advanceTimersByTimeAsync(50);

    await rejection;
    expect(signal?.aborted).toBe(true);
    expect(runtime.getSnapshot().activeThemeId).toBe(BUILTIN_THEME_ID);
    vi.useRealTimers();
  });

  it("publishes the new theme before it deactivates the old theme", async () => {
    const { runtime } = createRuntime();
    const events: string[] = [];
    const unsubscribe = runtime.subscribe(() => {
      events.push(`snapshot:${runtime.getSnapshot().activeThemeId}`);
    });
    await runtime.prepareTheme(manifest("com.example.first"), {
      id: "com.example.first",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup() {
        return { deactivate() { events.push("first.deactivate"); } };
      },
    });
    await runtime.activateTheme("com.example.first");
    events.length = 0;
    await runtime.prepareTheme(manifest("com.example.second"), {
      id: "com.example.second",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup() {
        return { activate() { events.push("second.activate"); } };
      },
    });
    events.length = 0;

    await runtime.activateTheme("com.example.second");
    unsubscribe();

    expect(events).toEqual([
      "second.activate",
      "snapshot:com.example.second",
      "first.deactivate",
    ]);
  });

  it("reloads an active theme and disposes the previous generation", async () => {
    const { runtime, routes } = createRuntime();
    const events: string[] = [];
    const oldDefinition: ThemeDefinition = {
      id: "com.example.nebula",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup(context) {
        context.routes.override({ id: "old.home", target: "core.home", mode: "replace", component: EmptyPage });
        return {
          deactivate() { events.push("old.deactivate"); },
          dispose() { events.push("old.dispose"); },
        };
      },
    };
    await runtime.prepareTheme(manifest(oldDefinition.id), oldDefinition);
    await runtime.activateTheme(oldDefinition.id);

    await runtime.reloadTheme(manifest(oldDefinition.id, "1.1.0"), {
      ...oldDefinition,
      version: "1.1.0",
      setup(context) {
        context.routes.override({ id: "new.home", target: "core.home", mode: "replace", component: EmptyPage });
      },
    });

    const snapshot = runtime.getSnapshot();
    expect(snapshot.activeThemeId).toBe(oldDefinition.id);
    expect(routes.resolveCoreRoute("core.home", snapshot.activeOwner).replacement?.id).toBe("new.home");
    expect(events).toEqual(["old.deactivate", "old.dispose"]);
  });

  it("falls back to the built-in theme before it disposes an active theme", async () => {
    const { runtime } = createRuntime();
    const events: string[] = [];
    await runtime.prepareTheme(manifest("com.example.nebula"), {
      id: "com.example.nebula",
      version: "1.0.0",
      apiVersion: "1.0.0",
      setup() {
        return {
          deactivate() { events.push("deactivate"); },
          dispose() { events.push("dispose"); },
        };
      },
    });
    await runtime.activateTheme("com.example.nebula");

    await runtime.disposeTheme("com.example.nebula");

    expect(runtime.getSnapshot().activeThemeId).toBe(BUILTIN_THEME_ID);
    expect(runtime.getSnapshot().preparedThemeIds).toEqual([]);
    expect(events).toEqual(["deactivate", "dispose"]);
  });

  it("uses structured errors for unknown themes and definition mismatches", async () => {
    const { runtime } = createRuntime();
    await expect(runtime.activateTheme("com.example.missing")).rejects.toBeInstanceOf(
      ThemeRuntimeError,
    );
    await expect(
      runtime.prepareTheme(manifest("com.example.nebula"), {
        id: "com.example.other",
        version: "1.0.0",
        apiVersion: "1.0.0",
        setup() {},
      }),
    ).rejects.toMatchObject({ code: "THEME_DEFINITION_MISMATCH" });
  });
});
