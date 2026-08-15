import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { ThemeRuntimeContextValue } from "@/components/themes/theme-runtime-provider";
import { ThemeSwitcher, type ThemeSwitcherOperations } from "./theme-switcher";

const update = vi.fn();
let runtime: ThemeRuntimeContextValue;

vi.mock("@/components/settings/settings-provider", () => ({
  useSettings: () => ({
    settings: { appearance: { themeId: runtime.snapshot.activeThemeId } },
    update,
  }),
}));
vi.mock("@/components/themes/theme-runtime-provider", () => ({
  useThemeRuntime: () => runtime,
}));
vi.mock("@/components/i18n/use-i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

function createRuntime(activeThemeId = "builtin.default"): ThemeRuntimeContextValue {
  return {
    snapshot: {
      activeThemeId,
      activeOwner: activeThemeId,
      preparedThemeIds: [],
      revision: 0,
    },
    ready: true,
    error: null,
    packages: [{
      manifest: {
        schemaVersion: "1.0",
        id: "com.example.nebula",
        name: "Nebula",
        version: "1.0.0",
        author: { name: "Example" },
        engines: { rtlauncher: ">=0.2.0", themeApi: "^1.0.0" },
        entry: { script: "dist/theme.js" },
        supports: { colorSchemes: ["light", "dark"] },
      },
      development: false,
      location: "package",
    }],
    routes: {} as never,
    slots: {} as never,
    activateTheme: vi.fn(async () => true),
    reloadTheme: vi.fn(async () => true),
    refreshThemes: vi.fn(async () => undefined),
    reportThemeError: vi.fn(),
  };
}

function operations(
  overrides: Partial<ThemeSwitcherOperations> = {},
): ThemeSwitcherOperations {
  return {
    pickArchive: vi.fn(async () => null),
    pickDevelopmentDirectory: vi.fn(async () => null),
    installArchive: vi.fn(async () => undefined),
    registerDevelopmentDirectory: vi.fn(async () => undefined),
    removeTheme: vi.fn(async () => undefined),
    isTrusted: vi.fn(async () => false),
    setTrusted: vi.fn(async () => undefined),
    confirm: vi.fn(async () => true),
    ...overrides,
  };
}

beforeEach(() => {
  const values = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
    clear: () => values.clear(),
  });
});

afterEach(() => {
  cleanup();
  update.mockReset();
  vi.unstubAllGlobals();
});

describe("ThemeSwitcher", () => {
  it("does not offer removal for the built-in Theme", () => {
    runtime = createRuntime();
    render(<ThemeSwitcher operations={operations()} />);

    expect(screen.queryByRole("button", { name: "settings.themeManager.remove" })).toBeNull();
  });

  it("requires trust confirmation before it activates local code", async () => {
    runtime = createRuntime();
    const actions = operations();
    render(<ThemeSwitcher operations={actions} />);

    fireEvent.change(screen.getByLabelText("settings.themeManager.selectTheme"), {
      target: { value: "com.example.nebula" },
    });

    await waitFor(() => {
      expect(actions.confirm).toHaveBeenCalledOnce();
      expect(actions.isTrusted).toHaveBeenCalledWith("com.example.nebula", "1.0.0");
      expect(actions.setTrusted).toHaveBeenCalledWith(
        "com.example.nebula",
        "1.0.0",
        true,
      );
      expect(runtime.activateTheme).toHaveBeenCalledWith("com.example.nebula");
      expect(update).toHaveBeenCalledWith("appearance", { themeId: "com.example.nebula" });
    });
  });

  it("shows switch failure feedback and keeps the saved Theme", async () => {
    runtime = createRuntime();
    runtime.activateTheme = vi.fn(async () => false);
    const actions = operations();
    render(<ThemeSwitcher operations={actions} />);

    fireEvent.change(screen.getByLabelText("settings.themeManager.selectTheme"), {
      target: { value: "com.example.nebula" },
    });

    expect((await screen.findByRole("alert")).textContent).toBe(
      "settings.themeManager.switchFailed",
    );
    expect(update).not.toHaveBeenCalled();
  });

  it("installs an archive and refreshes the package list", async () => {
    runtime = createRuntime();
    const actions = operations({ pickArchive: vi.fn(async () => "/tmp/nebula.rtltheme") });
    render(<ThemeSwitcher operations={actions} />);

    fireEvent.click(screen.getByRole("button", { name: "settings.themeManager.install" }));

    await waitFor(() => {
      expect(actions.installArchive).toHaveBeenCalledWith("/tmp/nebula.rtltheme");
      expect(runtime.refreshThemes).toHaveBeenCalledOnce();
    });
  });
});
