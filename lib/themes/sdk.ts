import type {
  JsonValue,
  RTLauncherThemeSDK,
  ThemeDownloadTask,
  ThemePlatformInfo,
} from "./protocol";
import { ThemeSDKError } from "./protocol";
import type { ThemeEventBus } from "./events";

export interface ThemeSDKAccountSource {
  id: string;
  name: string;
  uuid?: string;
  authType: string;
  skinUrl?: string | null;
  accessToken?: string;
  yggdrasilUrl?: string;
}

export interface ThemeSDKDependencies {
  invoke<T>(command: string, args?: Readonly<Record<string, unknown>>): Promise<T>;
  accounts: {
    list(): readonly ThemeSDKAccountSource[];
    getActive(): ThemeSDKAccountSource | null;
    setActive(accountId: string): Promise<void>;
  };
  instances: { select(instanceId: string): Promise<void> };
  launch: {
    start(overrides?: Readonly<Record<string, JsonValue>>): Promise<void>;
    stop(): Promise<void>;
    getStatus(): string;
  };
  downloads: {
    list(): readonly ThemeDownloadTask[];
    cancel(taskId: string | number): Promise<void>;
  };
  router: {
    navigate(target: string): Promise<void>;
    back(): void;
    getLocation(): string;
  };
  settings: {
    get(): Readonly<Record<string, JsonValue>>;
    update(patch: Readonly<Record<string, JsonValue>>): Promise<void>;
    subscribe(listener: () => void): () => void;
  };
  i18n: { t(key: string): string; getLocale(): string };
  ui: { toast(message: string): string; confirm(message: string): Promise<boolean> };
  storage: Storage;
  events: ThemeEventBus;
  platform: ThemePlatformInfo;
}

interface NativeInstanceSummary {
  name: string;
  minecraft_version: string;
  loader: string;
  mods_count: number;
}

const STORAGE_KEY_PATTERN = /^[a-zA-Z0-9._-]+$/;

function toAccountSummary(account: ThemeSDKAccountSource) {
  return {
    id: account.id,
    name: account.name,
    uuid: account.uuid,
    authType: account.authType,
    skinUrl: account.skinUrl ?? undefined,
  };
}

function storageKey(themeId: string, key: string): string {
  if (!STORAGE_KEY_PATTERN.test(key)) {
    throw new ThemeSDKError(
      "THEME_STORAGE_KEY_INVALID",
      "Theme storage keys can contain letters, numbers, dots, underscores, and hyphens.",
    );
  }
  return `rtlauncher:theme:${themeId}:${key}`;
}

export function createThemeSDK(
  themeId: string,
  dependencies: ThemeSDKDependencies,
): RTLauncherThemeSDK {
  return {
    accounts: {
      async list() {
        return dependencies.accounts.list().map(toAccountSummary);
      },
      async getActive() {
        const active = dependencies.accounts.getActive();
        return active ? toAccountSummary(active) : null;
      },
      async setActive(accountId) {
        await dependencies.accounts.setActive(accountId);
      },
    },
    instances: {
      async list(instancesPath) {
        if (!instancesPath?.trim()) {
          throw new ThemeSDKError(
            "THEME_SDK_ARGUMENT_INVALID",
            "instancesPath is required to list instances.",
          );
        }
        const instances = await dependencies.invoke<NativeInstanceSummary[]>(
          "vm_scan_instances",
          { instancesPath },
        );
        return instances.map((instance) => ({
          name: instance.name,
          minecraftVersion: instance.minecraft_version,
          loader: instance.loader,
          modsCount: instance.mods_count,
        }));
      },
      async select(instanceId) {
        await dependencies.instances.select(instanceId);
      },
    },
    launch: {
      async start(overrides) { await dependencies.launch.start(overrides); },
      async stop() { await dependencies.launch.stop(); },
      getStatus() { return dependencies.launch.getStatus(); },
    },
    downloads: {
      list() { return dependencies.downloads.list(); },
      async cancel(taskId) { await dependencies.downloads.cancel(taskId); },
    },
    router: {
      async navigate(target) { await dependencies.router.navigate(target); },
      back() { dependencies.router.back(); },
      getLocation() { return dependencies.router.getLocation(); },
    },
    settings: {
      get() { return dependencies.settings.get(); },
      async update(patch) { await dependencies.settings.update(patch); },
      subscribe(listener) { return dependencies.settings.subscribe(listener); },
    },
    i18n: {
      t(key) { return dependencies.i18n.t(key); },
      getLocale() { return dependencies.i18n.getLocale(); },
    },
    ui: {
      toast(message) { return dependencies.ui.toast(message); },
      async confirm(message) { return dependencies.ui.confirm(message); },
    },
    storage: {
      async get<T extends JsonValue>(key: string): Promise<T | null> {
        const raw = dependencies.storage.getItem(storageKey(themeId, key));
        if (raw === null) return null;
        try {
          return JSON.parse(raw) as T;
        } catch (error) {
          throw new ThemeSDKError(
            "THEME_STORAGE_VALUE_INVALID",
            `Stored value for ${key} is not valid JSON.`,
            { cause: error },
          );
        }
      },
      async set(key, value) {
        dependencies.storage.setItem(storageKey(themeId, key), JSON.stringify(value));
      },
      async remove(key) {
        dependencies.storage.removeItem(storageKey(themeId, key));
      },
    },
    events: dependencies.events.forOwner(themeId),
    platform: Object.freeze({ ...dependencies.platform }),
    unsafe: {
      async invoke<T>(command: string, args?: Readonly<Record<string, unknown>>) {
        try {
          return await dependencies.invoke<T>(command, args);
        } catch (error) {
          throw new ThemeSDKError(
            "THEME_UNSAFE_INVOKE_FAILED",
            `Tauri command ${command} failed.`,
            { cause: error },
          );
        }
      },
    },
  };
}
