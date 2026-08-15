"use client";

import { invoke } from "@tauri-apps/api/core";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { usePathname, useRouter } from "next/navigation";

import { useAccountContext } from "@/components/accounts/account-provider";
import { useDownloadManager } from "@/components/download/download-provider";
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { useSettings } from "@/components/settings/settings-provider";
import {
  loadThemeBundle,
  nativeThemeBundleReader,
  type LoadedThemeBundle,
} from "@/lib/themes/bundle-loader";
import { ThemeEventBus } from "@/lib/themes/events";
import type {
  JsonValue,
  ThemeAssetService,
  ThemeContext,
  ThemeManifest,
  ThemeSettingsService,
} from "@/lib/themes/protocol";
import { BUILTIN_THEME_ID, THEME_API_VERSION } from "@/lib/themes/protocol";
import { ThemeRouteRegistry } from "@/lib/themes/route-registry";
import {
  ThemeRuntime,
  type ThemeContextServices,
  type ThemeRuntimeSnapshot,
} from "@/lib/themes/runtime";
import { createThemeSDK } from "@/lib/themes/sdk";
import { ThemeSlotRegistry } from "@/lib/themes/slot-registry";

export interface ThemeInstalledPackage {
  manifest: ThemeManifest;
  development: boolean;
  location: string;
}

export interface ThemeStoreView {
  activeThemeId: string;
  lastHealthyThemeId: string;
  pendingThemeId: string | null;
  packages: ThemeInstalledPackage[];
}

export interface ThemeHostDependencies {
  loadStore(): Promise<ThemeStoreView>;
  loadBundle(manifest: ThemeManifest): Promise<LoadedThemeBundle>;
  setActive(themeId: string): Promise<void>;
  markHealthy(themeId: string): Promise<void>;
  createContextServices(
    manifest: ThemeManifest,
    assets: ThemeAssetService,
  ): ThemeContextServices;
}

export interface ThemeRuntimeContextValue {
  snapshot: ThemeRuntimeSnapshot;
  ready: boolean;
  error: Error | null;
  packages: readonly ThemeInstalledPackage[];
  routes: ThemeRouteRegistry;
  slots: ThemeSlotRegistry;
  activateTheme(themeId: string): Promise<boolean>;
  reloadTheme(themeId: string): Promise<boolean>;
}

interface ThemeRuntimeProviderProps {
  children: ReactNode;
  dependencies?: ThemeHostDependencies;
}

const ThemeRuntimeContext = createContext<ThemeRuntimeContextValue | null>(null);

const EMPTY_ASSETS: ThemeAssetService = {
  async url() { throw new Error("Theme assets are not loaded."); },
  release() {},
};

function ThemeRuntimeProviderCore({
  children,
  dependencies,
}: Required<ThemeRuntimeProviderProps>) {
  const dependenciesRef = useRef(dependencies);
  dependenciesRef.current = dependencies;
  const routes = useMemo(() => new ThemeRouteRegistry(), []);
  const slots = useMemo(() => new ThemeSlotRegistry(), []);
  const bundles = useRef(new Map<string, LoadedThemeBundle>());
  const packagesRef = useRef<ThemeInstalledPackage[]>([]);
  const runtime = useMemo(() => new ThemeRuntime({
    appVersion: "1.0.0",
    platform: detectPlatform(),
    routes,
    slots,
    createContextServices(manifest) {
      const assets = bundles.current.get(manifest.id)?.assets ?? EMPTY_ASSETS;
      return dependenciesRef.current.createContextServices(manifest, assets);
    },
  }), [routes, slots]);
  const snapshot = useSyncExternalStore(
    (listener) => runtime.subscribe(listener),
    () => runtime.getSnapshot(),
    () => runtime.getSnapshot(),
  );
  const [packages, setPackages] = useState<ThemeInstalledPackage[]>([]);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const findPackage = useCallback((themeId: string) => {
    return packagesRef.current.find((item) => item.manifest.id === themeId);
  }, []);

  const prepareTheme = useCallback(async (themeId: string) => {
    if (runtime.getSnapshot().preparedThemeIds.includes(themeId)) return;
    const themePackage = findPackage(themeId);
    if (!themePackage) throw new Error(`Theme is not installed: ${themeId}`);
    const loaded = await dependenciesRef.current.loadBundle(themePackage.manifest);
    bundles.current.set(themeId, loaded);
    try {
      await runtime.prepareTheme(themePackage.manifest, loaded.definition);
    } catch (cause) {
      bundles.current.delete(themeId);
      loaded.unload();
      throw cause;
    }
  }, [findPackage, runtime]);

  const activateTheme = useCallback(async (themeId: string): Promise<boolean> => {
    const previousThemeId = runtime.getSnapshot().activeThemeId;
    if (themeId === previousThemeId) return true;
    setError(null);
    try {
      if (themeId !== BUILTIN_THEME_ID) await prepareTheme(themeId);
      await dependenciesRef.current.setActive(themeId);
      await runtime.activateTheme(themeId);
      await dependenciesRef.current.markHealthy(themeId);
      return true;
    } catch (cause) {
      const nextError = cause instanceof Error ? cause : new Error(String(cause));
      setError(nextError);
      try {
        if (runtime.getSnapshot().activeThemeId !== previousThemeId) {
          await runtime.activateTheme(previousThemeId);
        }
        await dependenciesRef.current.setActive(previousThemeId);
        await dependenciesRef.current.markHealthy(previousThemeId);
      } catch {}
      return false;
    }
  }, [prepareTheme, runtime]);

  const reloadTheme = useCallback(async (themeId: string): Promise<boolean> => {
    const themePackage = findPackage(themeId);
    if (!themePackage) {
      setError(new Error(`Theme is not installed: ${themeId}`));
      return false;
    }
    const previous = bundles.current.get(themeId);
    try {
      const loaded = await dependenciesRef.current.loadBundle(themePackage.manifest);
      bundles.current.set(themeId, loaded);
      try {
        await runtime.reloadTheme(themePackage.manifest, loaded.definition);
      } catch (cause) {
        if (previous) bundles.current.set(themeId, previous);
        else bundles.current.delete(themeId);
        loaded.unload();
        throw cause;
      }
      previous?.unload();
      setError(null);
      return true;
    } catch (cause) {
      setError(cause instanceof Error ? cause : new Error(String(cause)));
      return false;
    }
  }, [findPackage, runtime]);

  useEffect(() => {
    let cancelled = false;
    void dependenciesRef.current.loadStore().then(async (store) => {
      if (cancelled) return;
      packagesRef.current = store.packages;
      setPackages(store.packages);
      if (store.activeThemeId !== BUILTIN_THEME_ID) {
        try {
          await prepareTheme(store.activeThemeId);
          if (!cancelled) await runtime.activateTheme(store.activeThemeId);
        } catch (cause) {
          if (!cancelled) {
            setError(cause instanceof Error ? cause : new Error(String(cause)));
            try {
              await dependenciesRef.current.setActive(BUILTIN_THEME_ID);
              await dependenciesRef.current.markHealthy(BUILTIN_THEME_ID);
            } catch {}
          }
        }
      }
    }).catch((cause) => {
      if (!cancelled) setError(cause instanceof Error ? cause : new Error(String(cause)));
    }).finally(() => {
      if (!cancelled) setReady(true);
    });
    return () => {
      cancelled = true;
      bundles.current.forEach((bundle) => bundle.unload());
      bundles.current.clear();
    };
  }, [prepareTheme, runtime]);

  useEffect(() => {
    document.documentElement.setAttribute("data-rtl-theme", snapshot.activeThemeId);
    return () => document.documentElement.removeAttribute("data-rtl-theme");
  }, [snapshot.activeThemeId]);

  const value = useMemo<ThemeRuntimeContextValue>(() => ({
    snapshot,
    ready,
    error,
    packages,
    routes,
    slots,
    activateTheme,
    reloadTheme,
  }), [activateTheme, error, packages, ready, reloadTheme, routes, slots, snapshot]);

  return <ThemeRuntimeContext.Provider value={value}>{children}</ThemeRuntimeContext.Provider>;
}

export function ThemeRuntimeProvider(props: ThemeRuntimeProviderProps) {
  if (props.dependencies) {
    return <ThemeRuntimeProviderCore {...props} dependencies={props.dependencies} />;
  }
  return <ConnectedThemeRuntimeProvider>{props.children}</ConnectedThemeRuntimeProvider>;
}

function ConnectedThemeRuntimeProvider({ children }: { children: ReactNode }) {
  const accounts = useAccountContext();
  const downloads = useDownloadManager();
  const launch = useLaunchContext();
  const settings = useSettings();
  const { t, language } = useI18n();
  const router = useRouter();
  const pathname = usePathname();
  const eventBus = useMemo(() => new ThemeEventBus(), []);
  const current = useRef({ accounts, downloads, launch, settings, t, language, router, pathname });
  current.current = { accounts, downloads, launch, settings, t, language, router, pathname };

  const dependencies = useMemo<ThemeHostDependencies>(() => ({
    loadStore: () => invoke<ThemeStoreView>("theme_list"),
    loadBundle: (themeManifest) => loadThemeBundle(themeManifest, {
      reader: nativeThemeBundleReader,
    }),
    setActive: (themeId) => invoke("theme_set_active", { themeId }),
    markHealthy: (themeId) => invoke("theme_mark_healthy", { themeId }),
    createContextServices(themeManifest, assets) {
      const themeSettings = createThemeSettingsService(themeManifest.id);
      const sdk = createThemeSDK(themeManifest.id, {
        invoke,
        accounts: {
          list: () => current.current.accounts.profiles,
          getActive: () => current.current.accounts.selectedProfile,
          async setActive(accountId) {
            const account = current.current.accounts.profiles.find((item) => item.id === accountId);
            if (!account) throw new Error(`Account is not available: ${accountId}`);
            current.current.accounts.selectProfile(account);
          },
        },
        instances: {
          async select(instanceId) {
            current.current.launch.updateConfig({ versionName: instanceId });
          },
        },
        launch: {
          start: (overrides) => current.current.launch.launchGame(overrides as never),
          stop: () => current.current.launch.cancelLaunch(),
          getStatus: () => current.current.launch.status,
        },
        downloads: {
          list: () => current.current.downloads.tasks.map((task) => ({
            id: task.taskId,
            label: task.label,
            status: task.status,
            progress: task.progress,
          })),
          cancel: (taskId) => current.current.downloads.cancelDownload(Number(taskId)),
        },
        router: {
          async navigate(target) { current.current.router.push(target); },
          back() { current.current.router.back(); },
          getLocation: () => current.current.pathname,
        },
        settings: {
          get: () => current.current.settings.settings as unknown as Readonly<Record<string, JsonValue>>,
          async update(patch) {
            if (patch.general && typeof patch.general === "object" && !Array.isArray(patch.general)) {
              current.current.settings.update("general", patch.general as never);
            }
            if (patch.appearance && typeof patch.appearance === "object" && !Array.isArray(patch.appearance)) {
              current.current.settings.update("appearance", patch.appearance as never);
            }
          },
          subscribe: () => () => undefined,
        },
        i18n: {
          t: (key) => current.current.t(key as TranslationKey),
          getLocale: () => current.current.language,
        },
        ui: {
          toast(message) {
            (window as Window & { __rt_toast?: (value: string) => void })
              .__rt_toast?.(message);
            return `${Date.now()}`;
          },
          async confirm(message) { return window.confirm(message); },
        },
        storage: localStorage,
        events: eventBus,
        platform: {
          os: detectPlatform(),
          appVersion: "1.0.0",
          themeApiVersion: THEME_API_VERSION,
        },
      });
      return {
        sdk,
        assets,
        settings: themeSettings,
        events: eventBus.forOwner(themeManifest.id),
        logger: createThemeLogger(themeManifest.id),
      };
    },
  }), [eventBus]);

  return (
    <ThemeRuntimeProviderCore dependencies={dependencies}>
      {children}
    </ThemeRuntimeProviderCore>
  );
}

function createThemeSettingsService(themeId: string): ThemeSettingsService {
  const key = `rtlauncher:theme-settings:${themeId}`;
  const listeners = new Set<(value: JsonValue) => void>();
  const read = (): JsonValue => {
    const stored = localStorage.getItem(key);
    return stored ? JSON.parse(stored) as JsonValue : {};
  };
  return {
    async get<T extends JsonValue>() { return read() as T; },
    async update<T extends JsonValue>(value: T) {
      localStorage.setItem(key, JSON.stringify(value));
      listeners.forEach((listener) => listener(value));
      return value;
    },
    async reset() {
      localStorage.removeItem(key);
      listeners.forEach((listener) => listener({}));
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    registerMigration() { return () => undefined; },
  };
}

function createThemeLogger(themeId: string): ThemeContext["logger"] {
  const prefix = `[Theme ${themeId}]`;
  return {
    debug(message, details) { console.debug(prefix, message, details ?? ""); },
    info(message, details) { console.info(prefix, message, details ?? ""); },
    warn(message, details) { console.warn(prefix, message, details ?? ""); },
    error(message, details) { console.error(prefix, message, details ?? ""); },
  };
}

function detectPlatform(): "windows" | "macos" | "linux" {
  if (typeof navigator === "undefined") return "linux";
  const platform = navigator.userAgent.toLowerCase();
  if (platform.includes("win")) return "windows";
  if (platform.includes("mac")) return "macos";
  return "linux";
}

export function useThemeRuntime(): ThemeRuntimeContextValue {
  const value = useContext(ThemeRuntimeContext);
  if (!value) throw new Error("useThemeRuntime must be used within ThemeRuntimeProvider.");
  return value;
}
