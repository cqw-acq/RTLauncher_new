import type { ComponentType, ReactNode } from "react";

export const THEME_SCHEMA_VERSION = "1.0" as const;
export const THEME_API_VERSION = "1.0.0" as const;
export const BUILTIN_THEME_ID = "builtin.default" as const;

export const CORE_ROUTE_IDS = [
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
] as const;

export type CoreRouteId = (typeof CORE_ROUTE_IDS)[number];

export const CORE_SLOT_IDS = [
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
] as const;

export type CoreSlotId = (typeof CORE_SLOT_IDS)[number];

const CORE_ROUTE_ID_SET = new Set<string>(CORE_ROUTE_IDS);
const CORE_SLOT_ID_SET = new Set<string>(CORE_SLOT_IDS);

export function isCoreRouteId(value: string): value is CoreRouteId {
  return CORE_ROUTE_ID_SET.has(value);
}

export function isCoreSlotId(value: string): value is CoreSlotId {
  return CORE_SLOT_ID_SET.has(value);
}

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue =
  | JsonPrimitive
  | { readonly [key: string]: JsonValue }
  | readonly JsonValue[];

export interface ThemeAuthor {
  name: string;
  url?: string;
}

export interface ThemeEngineRequirements {
  rtlauncher: string;
  themeApi: string;
  themeUi?: string;
}

export interface ThemeEntryPoints {
  script: string;
  style?: string;
}

export type ThemeColorScheme = "light" | "dark";
export type ThemeUserOverride =
  | "accentColor"
  | "fontSize"
  | "backgroundImage";

export interface ThemeSupportDeclaration {
  colorSchemes: readonly ThemeColorScheme[];
  locales?: readonly string[];
  userOverrides?: readonly ThemeUserOverride[];
}

export type ThemeRouteMode = "replace" | "wrap";
export type ThemeSlotMode = "replace" | "before" | "after" | "wrap";

export interface ThemeRouteManifestContribution {
  id: string;
  target?: CoreRouteId;
  path?: string;
  mode: ThemeRouteMode;
}

export interface ThemeSlotManifestContribution {
  id: string;
  target: CoreSlotId;
  mode: ThemeSlotMode;
  order?: number;
}

export interface ThemeSettingsManifestContribution {
  schema: string;
  defaults?: string;
}

export interface ThemeContributions {
  routes?: readonly ThemeRouteManifestContribution[];
  slots?: readonly ThemeSlotManifestContribution[];
  settings?: ThemeSettingsManifestContribution;
}

export interface ThemeIntegrity {
  algorithm: "sha256";
  files: Readonly<Record<string, string>>;
}

export interface ThemeManifest {
  schemaVersion: string;
  id: string;
  name: string;
  version: string;
  description?: string;
  author: ThemeAuthor;
  license?: string;
  homepage?: string;
  icon?: string;
  previews?: readonly string[];
  engines: ThemeEngineRequirements;
  entry: ThemeEntryPoints;
  supports: ThemeSupportDeclaration;
  contributes?: ThemeContributions;
  disclosures?: readonly string[];
  integrity?: ThemeIntegrity;
  extensions?: Readonly<Record<string, JsonValue>>;
}

export interface ThemeRouteComponentProps {
  routeId: string;
  params: Readonly<Record<string, string>>;
  search: URLSearchParams;
  children?: ReactNode;
}

export interface ThemeSlotComponentProps<T = unknown> {
  slotId: CoreSlotId;
  data: T;
  children?: ReactNode;
}

export interface ThemeRouteRegistration {
  id: string;
  target: CoreRouteId;
  mode: ThemeRouteMode;
  component: ComponentType<ThemeRouteComponentProps>;
}

export interface ThemePageRegistration {
  id: string;
  path: string;
  component: ComponentType<ThemeRouteComponentProps>;
}

export interface ThemeSlotRegistration<T = unknown> {
  id: string;
  target: CoreSlotId;
  mode: ThemeSlotMode;
  order?: number;
  component: ComponentType<ThemeSlotComponentProps<T>>;
}

export interface ThemeRouteRegistryAPI {
  override(registration: ThemeRouteRegistration): () => void;
  add(registration: ThemePageRegistration): () => void;
}

export interface ThemeSlotRegistryAPI {
  register<T = unknown>(registration: ThemeSlotRegistration<T>): () => void;
}

export type ThemeUnsubscribe = () => void;

export interface ThemeAssetService {
  url(path: string): Promise<string>;
  release(url: string): void;
}

export interface ThemeSettingsMigration {
  from: string;
  to: string;
  migrate(value: JsonValue): JsonValue | Promise<JsonValue>;
}

export interface ThemeSettingsService {
  get<T extends JsonValue = JsonValue>(): Promise<T>;
  update<T extends JsonValue = JsonValue>(value: T): Promise<T>;
  reset(): Promise<void>;
  subscribe(listener: (value: JsonValue) => void): ThemeUnsubscribe;
  registerMigration(migration: ThemeSettingsMigration): ThemeUnsubscribe;
}

export interface ThemeEventService {
  emit<T extends JsonValue>(name: string, payload: T): void;
  on<T extends JsonValue>(
    name: string,
    listener: (payload: T) => void,
  ): ThemeUnsubscribe;
}

export interface ThemeLogger {
  debug(message: string, details?: JsonValue): void;
  info(message: string, details?: JsonValue): void;
  warn(message: string, details?: JsonValue): void;
  error(message: string, details?: JsonValue): void;
}

export interface ThemeRuntimeInfo {
  appVersion: string;
  themeApiVersion: string;
  platform: "windows" | "macos" | "linux";
  development: boolean;
  activeThemeId: string;
}

export interface ThemeAccountSummary {
  id: string;
  name: string;
  uuid?: string;
  authType: string;
  skinUrl?: string;
}

export interface ThemeInstanceSummary {
  name: string;
  minecraftVersion?: string;
  loader?: string;
  path?: string;
  modsCount?: number;
}

export interface ThemeDownloadTask {
  id: string | number;
  label: string;
  status: string;
  progress?: number;
}

export interface ThemePlatformInfo {
  os: "windows" | "macos" | "linux";
  appVersion: string;
  themeApiVersion: string;
}

export interface RTLauncherThemeSDK {
  accounts: {
    list(): Promise<readonly ThemeAccountSummary[]>;
    getActive(): Promise<ThemeAccountSummary | null>;
    setActive(accountId: string): Promise<void>;
  };
  instances: {
    list(instancesPath?: string): Promise<readonly ThemeInstanceSummary[]>;
    select(instanceId: string): Promise<void>;
  };
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
    subscribe(listener: () => void): ThemeUnsubscribe;
  };
  i18n: {
    t(key: string): string;
    getLocale(): string;
  };
  ui: {
    toast(message: string): string;
    confirm(message: string): Promise<boolean>;
  };
  storage: {
    get<T extends JsonValue = JsonValue>(key: string): Promise<T | null>;
    set(key: string, value: JsonValue): Promise<void>;
    remove(key: string): Promise<void>;
  };
  events: ThemeEventService;
  platform: ThemePlatformInfo;
  unsafe: {
    invoke<T>(
      command: string,
      args?: Readonly<Record<string, unknown>>,
    ): Promise<T>;
  };
}

export interface ThemeContext {
  readonly manifest: ThemeManifest;
  readonly runtime: ThemeRuntimeInfo;
  readonly sdk: RTLauncherThemeSDK;
  readonly routes: ThemeRouteRegistryAPI;
  readonly slots: ThemeSlotRegistryAPI;
  readonly assets: ThemeAssetService;
  readonly settings: ThemeSettingsService;
  readonly events: ThemeEventService;
  readonly logger: ThemeLogger;
}

export interface ThemeActivateEvent {
  previousThemeId: string;
  signal: AbortSignal;
}

export interface ThemeDeactivateEvent {
  nextThemeId: string;
  signal: AbortSignal;
}

export interface ThemeLifecycle {
  activate?(event: ThemeActivateEvent): void | Promise<void>;
  deactivate?(event: ThemeDeactivateEvent): void | Promise<void>;
  dispose?(): void | Promise<void>;
}

export interface ThemeDefinition {
  id: string;
  version: string;
  apiVersion: string;
  setup(context: ThemeContext): void | ThemeLifecycle | Promise<void | ThemeLifecycle>;
}

export class ThemeSDKError extends Error {
  readonly code: string;
  readonly retryable: boolean;
  readonly details?: Readonly<Record<string, unknown>>;

  constructor(
    code: string,
    message: string,
    options: {
      retryable?: boolean;
      details?: Readonly<Record<string, unknown>>;
      cause?: unknown;
    } = {},
  ) {
    super(message, { cause: options.cause });
    this.name = "ThemeSDKError";
    this.code = code;
    this.retryable = options.retryable ?? false;
    this.details = options.details;
  }
}
