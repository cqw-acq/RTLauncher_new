import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { ThemeDefinition, ThemeManifest } from "@/lib/themes/protocol";
import {
  ThemeRuntimeProvider,
  useThemeRuntime,
  type ThemeHostDependencies,
} from "./theme-runtime-provider";

function manifest(id: string): ThemeManifest {
  return {
    schemaVersion: "1.0",
    id,
    name: id,
    version: "1.0.0",
    author: { name: "Test" },
    engines: { rtlauncher: ">=0.2.0", themeApi: "^1.0.0" },
    entry: { script: "dist/theme.js" },
    supports: { colorSchemes: ["light", "dark"] },
  };
}

function definition(id: string, activate?: () => void): ThemeDefinition {
  return {
    id,
    version: "1.0.0",
    apiVersion: "1.0.0",
    setup() { return { activate }; },
  };
}

function dependencies(activeThemeId = "builtin.default"): ThemeHostDependencies {
  return {
    async loadStore() {
      return {
        activeThemeId,
        lastHealthyThemeId: activeThemeId,
        pendingThemeId: null,
        packages: activeThemeId === "builtin.default"
          ? []
          : [{ manifest: manifest(activeThemeId), development: false, location: "package" }],
      };
    },
    loadBundle: vi.fn(async (themeManifest: ThemeManifest) => ({
      definition: definition(themeManifest.id),
      assets: { async url(path: string) { return `blob:${path}`; }, release() {} },
      unload: vi.fn(),
    })),
    setActive: vi.fn(async () => undefined),
    markHealthy: vi.fn(async () => undefined),
    healthDelayMs: 0,
    createContextServices: () => ({
      sdk: {} as never,
      assets: { async url(path: string) { return path; }, release() {} },
      settings: {
        async get<T>() { return {} as T; },
        async update(value) { return value; },
        async reset() {},
        subscribe() { return () => undefined; },
        registerMigration() { return () => undefined; },
      },
      events: { emit() {}, on() { return () => undefined; } },
      logger: { debug() {}, info() {}, warn() {}, error() {} },
    }),
  };
}

function Probe() {
  const theme = useThemeRuntime();
  return (
    <>
      <output data-testid="active">{theme.snapshot.activeThemeId}</output>
      <output data-testid="ready">{String(theme.ready)}</output>
      <button onClick={() => void theme.activateTheme("com.example.nebula")}>switch</button>
    </>
  );
}

afterEach(() => {
  cleanup();
  document.documentElement.removeAttribute("data-rtl-theme");
});

describe("ThemeRuntimeProvider", () => {
  it("starts with the built-in Theme", async () => {
    render(<ThemeRuntimeProvider dependencies={dependencies()}><Probe /></ThemeRuntimeProvider>);

    await waitFor(() => expect(screen.getByTestId("ready").textContent).toBe("true"));
    expect(screen.getByTestId("active").textContent).toBe("builtin.default");
  });

  it("loads the installed active Theme at startup", async () => {
    render(
      <ThemeRuntimeProvider dependencies={dependencies("com.example.nebula")}>
        <Probe />
      </ThemeRuntimeProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("active").textContent).toBe("com.example.nebula");
    });
  });

  it("switches immediately and records a healthy activation", async () => {
    const host = dependencies();
    host.loadStore = async () => ({
      activeThemeId: "builtin.default",
      lastHealthyThemeId: "builtin.default",
      pendingThemeId: null,
      packages: [{ manifest: manifest("com.example.nebula"), development: false, location: "package" }],
    });
    render(<ThemeRuntimeProvider dependencies={host}><Probe /></ThemeRuntimeProvider>);
    await waitFor(() => expect(screen.getByTestId("ready").textContent).toBe("true"));

    await act(async () => screen.getByRole("button", { name: "switch" }).click());

    expect(screen.getByTestId("active").textContent).toBe("com.example.nebula");
    expect(host.setActive).toHaveBeenCalledWith("com.example.nebula");
    await waitFor(() => expect(host.markHealthy).toHaveBeenCalledWith("com.example.nebula"));
  });

  it("keeps the previous Theme when activation fails", async () => {
    const host = dependencies();
    host.loadStore = async () => ({
      activeThemeId: "builtin.default",
      lastHealthyThemeId: "builtin.default",
      pendingThemeId: null,
      packages: [{ manifest: manifest("com.example.nebula"), development: false, location: "package" }],
    });
    host.loadBundle = vi.fn(async () => ({
      definition: definition("com.example.nebula", () => { throw new Error("broken"); }),
      assets: { async url(path: string) { return path; }, release() {} },
      unload() {},
    }));
    render(<ThemeRuntimeProvider dependencies={host}><Probe /></ThemeRuntimeProvider>);
    await waitFor(() => expect(screen.getByTestId("ready").textContent).toBe("true"));

    await act(async () => screen.getByRole("button", { name: "switch" }).click());

    expect(screen.getByTestId("active").textContent).toBe("builtin.default");
    await waitFor(() => {
      expect(host.markHealthy).toHaveBeenLastCalledWith("builtin.default");
    });
  });

  it("updates the root Theme attribute", async () => {
    render(
      <ThemeRuntimeProvider dependencies={dependencies("com.example.nebula")}>
        <Probe />
      </ThemeRuntimeProvider>,
    );

    await waitFor(() => {
      expect(document.documentElement.getAttribute("data-rtl-theme")).toBe(
        "com.example.nebula",
      );
    });
  });
});
