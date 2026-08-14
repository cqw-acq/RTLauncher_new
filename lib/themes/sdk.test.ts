import { beforeEach, describe, expect, it } from "vitest";

import type { JsonValue } from "./protocol";
import { ThemeSDKError } from "./protocol";
import { ThemeEventBus } from "./events";
import {
  createThemeSDK,
  type ThemeSDKDependencies,
} from "./sdk";

interface CapturedInvoke {
  command: string;
  args?: Readonly<Record<string, unknown>>;
}

class MemoryStorage implements Storage {
  private readonly values = new Map<string, string>();

  get length(): number { return this.values.size; }
  clear(): void { this.values.clear(); }
  getItem(key: string): string | null { return this.values.get(key) ?? null; }
  key(index: number): string | null { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string): void { this.values.delete(key); }
  setItem(key: string, value: string): void { this.values.set(key, value); }
}

let storage: MemoryStorage;

function dependencies(overrides: Partial<ThemeSDKDependencies> = {}): {
  deps: ThemeSDKDependencies;
  invokes: CapturedInvoke[];
} {
  const invokes: CapturedInvoke[] = [];
  const deps: ThemeSDKDependencies = {
    invoke: async <T>(command: string, args?: Readonly<Record<string, unknown>>) => {
      invokes.push({ command, args });
      return [] as T;
    },
    accounts: {
      list: () => [{
        id: "account-1",
        name: "Player",
        uuid: "uuid-1",
        authType: "microsoft",
        skinUrl: "skin.png",
        accessToken: "secret",
        yggdrasilUrl: "https://private.example",
      }],
      getActive: () => ({
        id: "account-1",
        name: "Player",
        uuid: "uuid-1",
        authType: "microsoft",
        accessToken: "secret",
      }),
      setActive: async () => undefined,
    },
    instances: {
      select: async () => undefined,
    },
    launch: {
      start: async () => undefined,
      stop: async () => undefined,
      getStatus: () => "idle",
    },
    downloads: {
      list: () => [],
      cancel: async () => undefined,
    },
    router: {
      navigate: async () => undefined,
      back: () => undefined,
      getLocation: () => "/",
    },
    settings: {
      get: () => ({}),
      update: async () => undefined,
      subscribe: () => () => undefined,
    },
    i18n: {
      t: (key) => key,
      getLocale: () => "en-US",
    },
    ui: {
      toast: () => "toast-1",
      confirm: async () => true,
    },
    storage,
    events: new ThemeEventBus(),
    platform: {
      os: "linux",
      appVersion: "0.2.0",
      themeApiVersion: "1.0.0",
    },
    ...overrides,
  };
  return { deps, invokes };
}

beforeEach(() => { storage = new MemoryStorage(); });

describe("createThemeSDK", () => {
  it("returns account summaries without credentials", async () => {
    const { deps } = dependencies();
    const sdk = createThemeSDK("com.example.nebula", deps);

    const accounts = await sdk.accounts.list();
    const active = await sdk.accounts.getActive();

    expect(accounts).toEqual([{
      id: "account-1",
      name: "Player",
      uuid: "uuid-1",
      authType: "microsoft",
      skinUrl: "skin.png",
    }]);
    expect(active).toEqual({
      id: "account-1",
      name: "Player",
      uuid: "uuid-1",
      authType: "microsoft",
      skinUrl: undefined,
    });
    expect(accounts[0]).not.toHaveProperty("accessToken");
    expect(accounts[0]).not.toHaveProperty("yggdrasilUrl");
  });

  it("maps instance scans to stable summary fields", async () => {
    const { deps, invokes } = dependencies({
      invoke: async <T>(command: string, args?: Readonly<Record<string, unknown>>) => {
        invokes.push({ command, args });
        return [{
          name: "Fabric 1.21",
          minecraft_version: "1.21",
          loader: "Fabric",
          mods_count: 12,
        }] as T;
      },
    });
    const sdk = createThemeSDK("com.example.nebula", deps);

    const instances = await sdk.instances.list("/minecraft/versions");

    expect(instances).toEqual([{
      name: "Fabric 1.21",
      minecraftVersion: "1.21",
      loader: "Fabric",
      modsCount: 12,
    }]);
    expect(invokes).toEqual([{
      command: "vm_scan_instances",
      args: { instancesPath: "/minecraft/versions" },
    }]);
  });

  it("stores JSON values under a Theme namespace", async () => {
    const { deps } = dependencies();
    const nebula = createThemeSDK("com.example.nebula", deps);
    const solar = createThemeSDK("com.example.solar", deps);

    await nebula.storage.set("layout", { compact: true });
    await solar.storage.set("layout", { compact: false });

    expect(await nebula.storage.get("layout")).toEqual({ compact: true });
    expect(await solar.storage.get("layout")).toEqual({ compact: false });
    expect(storage.getItem("rtlauncher:theme:com.example.nebula:layout")).toBe(
      '{"compact":true}',
    );
  });

  it("cleans owner subscriptions and restricts emitted event names", () => {
    const eventBus = new ThemeEventBus();
    const { deps } = dependencies({ events: eventBus });
    const sdk = createThemeSDK("com.example.nebula", deps);
    const received: JsonValue[] = [];
    sdk.events.on("com.example.nebula:panel.changed", (payload) => received.push(payload));

    sdk.events.emit("com.example.nebula:panel.changed", { panel: "home" });
    eventBus.clearOwner("com.example.nebula");
    sdk.events.emit("com.example.nebula:panel.changed", { panel: "settings" });

    expect(received).toEqual([{ panel: "home" }]);
    expect(() => sdk.events.emit("another.theme:changed", null)).toThrow(/namespace/i);
  });

  it("forwards launch operations through the stable adapter", async () => {
    const calls: string[] = [];
    const { deps } = dependencies({
      launch: {
        start: async () => { calls.push("start"); },
        stop: async () => { calls.push("stop"); },
        getStatus: () => "running",
      },
    });
    const sdk = createThemeSDK("com.example.nebula", deps);

    await sdk.launch.start();
    await sdk.launch.stop();

    expect(calls).toEqual(["start", "stop"]);
    expect(sdk.launch.getStatus()).toBe("running");
  });

  it("wraps unsafe invoke failures in a structured error", async () => {
    const cause = new Error("native failure");
    const { deps } = dependencies({
      invoke: async () => { throw cause; },
    });
    const sdk = createThemeSDK("com.example.nebula", deps);

    await expect(sdk.unsafe.invoke("custom_command")).rejects.toEqual(
      expect.objectContaining({
        name: "ThemeSDKError",
        code: "THEME_UNSAFE_INVOKE_FAILED",
        retryable: false,
        cause,
      } satisfies Partial<ThemeSDKError>),
    );
  });
});
