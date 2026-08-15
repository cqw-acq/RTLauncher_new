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
  type Dispatch,
  type ReactNode,
  type SetStateAction,
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
import { DevelopmentThemeWatcher } from "@/lib/themes/development-watcher";
import { ThemeHealthMonitor } from "@/lib/themes/health-monitor";
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
  healthDelayMs?: number;
  watchDevelopmentTheme?(
    themePackage: ThemeInstalledPackage,
    notifyChange: () => void,
  ): () => void;
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
  refreshThemes(): Promise<void>;
  reportThemeError(error: Error): void;
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

class ThemeRuntimeMutableState {
  packages: ThemeInstalledPackage[] = [];
  readonly bundles = new Map<string, LoadedThemeBundle>();
  activateTheme: (themeId: string) => Promise<boolean> = async () => false;

  constructor(public dependencies: ThemeHostDependencies) {}

  updateDependencies(dependencies: ThemeHostDependencies) {
    this.dependencies = dependencies;
  }

  updatePackages(packages: ThemeInstalledPackage[]) {
    this.packages = packages;
  }

  updateActivateTheme(activateTheme: (themeId: string) => Promise<boolean>) {
    this.activateTheme = activateTheme;
  }
}

function createRuntime(
  routes: ThemeRouteRegistry,
  slots: ThemeSlotRegistry,
  mutableState: ThemeRuntimeMutableState,
) {
  return new ThemeRuntime({
    appVersion: "1.0.0",
    platform: detectPlatform(),
    routes,
    slots,
    isDevelopmentTheme(manifest) {
      return mutableState.packages.some(
        (item) => item.development && item.manifest.id === manifest.id,
      );
    },
    createContextServices(manifest) {
      const assets = mutableState.bundles.get(manifest.id)?.assets ?? EMPTY_ASSETS;
      return mutableState.dependencies.createContextServices(manifest, assets);
    },
  });
}

function createHealthMonitor(
  healthyDelayMs: number | undefined,
  mutableState: ThemeRuntimeMutableState,
  setError: Dispatch<SetStateAction<Error | null>>,
) {
  return new ThemeHealthMonitor({
    healthyDelayMs,
    onHealthy(themeId) {
      return mutableState.dependencies.markHealthy(themeId).catch((cause) => {
        setError(cause instanceof Error ? cause : new Error(String(cause)));
      });
    },
    async onRollback() {
      await mutableState.activateTheme(BUILTIN_THEME_ID);
    },
  });
}

function ThemeRuntimeProviderCore({
  children,
  dependencies,
}: Required<ThemeRuntimeProviderProps>) {
  const [mutableState] = useState(() => new ThemeRuntimeMutableState(dependencies));
  const [routes] = useState(() => new ThemeRouteRegistry());
  const [slots] = useState(() => new ThemeSlotRegistry());
  const [runtime] = useState(() => createRuntime(routes, slots, mutableState));
  const snapshot = useSyncExternalStore(
    (listener) => runtime.subscribe(listener),
    () => runtime.getSnapshot(),
    () => runtime.getSnapshot(),
  );
  const [packages, setPackages] = useState<ThemeInstalledPackage[]>([]);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [healthMonitor] = useState(
    () => createHealthMonitor(dependencies.healthDelayMs, mutableState, setError),
  );

  useEffect(() => {
    mutableState.updateDependencies(dependencies);
  }, [dependencies, mutableState]);

  const refreshThemes = useCallback(async () => {
    const store = await mutableState.dependencies.loadStore();
    mutableState.updatePackages(store.packages);
    setPackages(store.packages);
  }, [mutableState]);

  const findPackage = useCallback((themeId: string) => {
    return mutableState.packages.find((item) => item.manifest.id === themeId);
  }, [mutableState]);

  const prepareTheme = useCallback(async (themeId: string) => {
    if (runtime.getSnapshot().preparedThemeIds.includes(themeId)) return;
    const themePackage = findPackage(themeId);
    if (!themePackage) throw new Error(`Theme is not installed: ${themeId}`);
    const loaded = await mutableState.dependencies.loadBundle(themePackage.manifest);
    mutableState.bundles.set(themeId, loaded);
    try {
      await runtime.prepareTheme(themePackage.manifest, loaded.definition);
    } catch (cause) {
      mutableState.bundles.delete(themeId);
      loaded.unload();
      throw cause;
    }
  }, [findPackage, mutableState, runtime]);

  const activateTheme = useCallback(async (themeId: string): Promise<boolean> => {
    const previousThemeId = runtime.getSnapshot().activeThemeId;
    if (themeId === previousThemeId) return true;
    setError(null);
    try {
      if (themeId !== BUILTIN_THEME_ID) await prepareTheme(themeId);
      await mutableState.dependencies.setActive(themeId);
      await runtime.activateTheme(themeId);
      healthMonitor.start(themeId);
      return true;
    } catch (cause) {
      const nextError = cause instanceof Error ? cause : new Error(String(cause));
      setError(nextError);
      try {
        if (runtime.getSnapshot().activeThemeId !== previousThemeId) {
          await runtime.activateTheme(previousThemeId);
        }
        await mutableState.dependencies.setActive(previousThemeId);
        healthMonitor.start(previousThemeId);
      } catch {}
      return false;
    }
  }, [healthMonitor, mutableState, prepareTheme, runtime]);

  useEffect(() => {
    mutableState.updateActivateTheme(activateTheme);
  }, [activateTheme, mutableState]);

  const reportThemeError = useCallback((themeError: Error) => {
    const activeThemeId = runtime.getSnapshot().activeThemeId;
    if (activeThemeId === BUILTIN_THEME_ID) return;
    console.error(`[Theme ${activeThemeId}] Runtime contribution failed.`, themeError);
    healthMonitor.reportError(activeThemeId);
  }, [healthMonitor, runtime]);

  const reloadTheme = useCallback(async (themeId: string): Promise<boolean> => {
    const themePackage = findPackage(themeId);
    if (!themePackage) {
      setError(new Error(`Theme is not installed: ${themeId}`));
      return false;
    }
    const previous = mutableState.bundles.get(themeId);
    try {
      const loaded = await mutableState.dependencies.loadBundle(themePackage.manifest);
      mutableState.bundles.set(themeId, loaded);
      try {
        await runtime.reloadTheme(themePackage.manifest, loaded.definition);
      } catch (cause) {
        if (previous) mutableState.bundles.set(themeId, previous);
        else mutableState.bundles.delete(themeId);
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
  }, [findPackage, mutableState, runtime]);

  useEffect(() => {
    let cancelled = false;
    void mutableState.dependencies.loadStore().then(async (store) => {
      if (cancelled) return;
      mutableState.updatePackages(store.packages);
      setPackages(store.packages);
      if (store.activeThemeId !== BUILTIN_THEME_ID) {
        try {
          await prepareTheme(store.activeThemeId);
          if (!cancelled) {
            await runtime.activateTheme(store.activeThemeId);
            if (store.pendingThemeId === store.activeThemeId) {
              healthMonitor.start(store.activeThemeId);
            }
          }
        } catch (cause) {
          if (!cancelled) {
            setError(cause instanceof Error ? cause : new Error(String(cause)));
            try {
              await mutableState.dependencies.setActive(BUILTIN_THEME_ID);
              await mutableState.dependencies.markHealthy(BUILTIN_THEME_ID);
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
      mutableState.bundles.forEach((bundle) => bundle.unload());
      mutableState.bundles.clear();
      healthMonitor.dispose();
    };
  }, [healthMonitor, mutableState, prepareTheme, runtime]);

  useEffect(() => {
    if (snapshot.activeThemeId === BUILTIN_THEME_ID) return;
    const onError = (event: ErrorEvent) => reportThemeError(
      event.error instanceof Error ? event.error : new Error(event.message),
    );
    const onRejection = (event: PromiseRejectionEvent) => reportThemeError(
      event.reason instanceof Error ? event.reason : new Error(String(event.reason)),
    );
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, [reportThemeError, snapshot.activeThemeId]);

  useEffect(() => {
    const activePackage = packages.find(
      (item) => item.development && item.manifest.id === snapshot.activeThemeId,
    );
    if (!activePackage || !mutableState.dependencies.watchDevelopmentTheme) return;
    const watcher = new DevelopmentThemeWatcher({
      async reload(themeId) {
        await refreshThemes();
        await reloadTheme(themeId);
      },
    });
    const stop = mutableState.dependencies.watchDevelopmentTheme(
      activePackage,
      () => watcher.notifyChange(activePackage.manifest.id),
    );
    return () => {
      stop();
      watcher.dispose();
    };
  }, [mutableState, packages, refreshThemes, reloadTheme, snapshot.activeThemeId]);

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
    refreshThemes,
    reportThemeError,
  }), [activateTheme, error, packages, ready, refreshThemes, reloadTheme, reportThemeError, routes, slots, snapshot]);

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

  useEffect(() => {
    current.current = { accounts, downloads, launch, settings, t, language, router, pathname };
  }, [accounts, downloads, language, launch, pathname, router, settings, t]);

  const dependencies = useMemo<ThemeHostDependencies>(() => ({
    loadStore: () => invoke<ThemeStoreView>("theme_list"),
    loadBundle: (themeManifest) => loadThemeBundle(themeManifest, {
      reader: nativeThemeBundleReader,
    }),
    setActive: (themeId) => invoke("theme_set_active", { themeId }),
    markHealthy: (themeId) => invoke("theme_mark_healthy", { themeId }),
    watchDevelopmentTheme: watchNativeDevelopmentTheme,
    createContextServices(themeManifest, assets) {
      const themeSettings = createThemeSettingsService(themeManifest.id);
      const sdk = createThemeSDK(themeManifest.id, {
        invoke,
        allowedUnsafeCommands: themeManifest.permissions?.unsafeCommands,
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

function watchNativeDevelopmentTheme(
  themePackage: ThemeInstalledPackage,
  notifyChange: () => void,
): () => void {
  let stopped = false;
  let previous: string | undefined;
  let reading = false;
  const read = async () => {
    if (stopped || reading) return;
    reading = true;
    try {
      const paths = [themePackage.manifest.entry.script];
      if (themePackage.manifest.entry.style) paths.push(themePackage.manifest.entry.style);
      const contents = await Promise.all(paths.map((path) => invoke<string>(
        "theme_read_text",
        { themeId: themePackage.manifest.id, path },
      )));
      const signature = contents.join("\u0000");
      if (previous !== undefined && signature !== previous) notifyChange();
      previous = signature;
    } catch {}
    finally { reading = false; }
  };
  void read();
  const interval = window.setInterval(() => void read(), 750);
  return () => {
    stopped = true;
    window.clearInterval(interval);
  };
}

export function useThemeRuntime(): ThemeRuntimeContextValue {
  const value = useContext(ThemeRuntimeContext);
  if (!value) throw new Error("useThemeRuntime must be used within ThemeRuntimeProvider.");
  return value;
}
