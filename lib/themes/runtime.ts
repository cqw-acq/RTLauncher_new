import type {
  ThemeContext,
  ThemeDefinition,
  ThemeLifecycle,
  ThemeManifest,
} from "./protocol";
import { BUILTIN_THEME_ID, THEME_API_VERSION } from "./protocol";
import type { ThemeRouteRegistry } from "./route-registry";
import type { ThemeSlotRegistry } from "./slot-registry";

export type ThemeContextServices = Omit<
  ThemeContext,
  "manifest" | "runtime" | "routes" | "slots"
>;

export interface ThemeRuntimeOptions {
  appVersion: string;
  platform: "windows" | "macos" | "linux";
  routes: ThemeRouteRegistry;
  slots: ThemeSlotRegistry;
  createContextServices(manifest: ThemeManifest): ThemeContextServices;
  isDevelopmentTheme?(manifest: ThemeManifest): boolean;
  setupTimeoutMs?: number;
  activateTimeoutMs?: number;
  deactivateTimeoutMs?: number;
}

export interface ThemeRuntimeSnapshot {
  activeThemeId: string;
  activeOwner: string;
  preparedThemeIds: readonly string[];
  revision: number;
}

export class ThemeRuntimeError extends Error {
  constructor(readonly code: string, message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "ThemeRuntimeError";
  }
}

interface PreparedTheme {
  definition: ThemeDefinition;
  lifecycle: ThemeLifecycle;
  manifest: ThemeManifest;
  owner: string;
  context: ThemeContext;
}

type ThemePhase = "setup" | "activation" | "deactivation" | "dispose";

const DEFAULT_SETUP_TIMEOUT_MS = 3_000;
const DEFAULT_ACTIVATE_TIMEOUT_MS = 3_000;
const DEFAULT_DEACTIVATE_TIMEOUT_MS = 2_000;

function majorVersion(value: string): number | null {
  const match = value.match(/^(\d+)\./);
  return match ? Number(match[1]) : null;
}

export class ThemeRuntime {
  private snapshot: ThemeRuntimeSnapshot = Object.freeze({
    activeThemeId: BUILTIN_THEME_ID,
    activeOwner: BUILTIN_THEME_ID,
    preparedThemeIds: [],
    revision: 0,
  });
  private readonly prepared = new Map<string, PreparedTheme>();
  private readonly listeners = new Set<() => void>();
  private readonly setupTimeoutMs: number;
  private readonly activateTimeoutMs: number;
  private readonly deactivateTimeoutMs: number;
  private ownerSequence = 0;

  constructor(private readonly options: ThemeRuntimeOptions) {
    this.setupTimeoutMs = options.setupTimeoutMs ?? DEFAULT_SETUP_TIMEOUT_MS;
    this.activateTimeoutMs = options.activateTimeoutMs ?? DEFAULT_ACTIVATE_TIMEOUT_MS;
    this.deactivateTimeoutMs = options.deactivateTimeoutMs ?? DEFAULT_DEACTIVATE_TIMEOUT_MS;
  }

  async prepareTheme(manifest: ThemeManifest, definition: ThemeDefinition): Promise<void> {
    const candidate = await this.prepareCandidate(manifest, definition);
    const previous = this.prepared.get(manifest.id);
    if (previous?.owner === this.snapshot.activeOwner) {
      await this.disposePrepared(candidate);
      throw new ThemeRuntimeError(
        "THEME_ALREADY_ACTIVE",
        `Theme ${manifest.id} is active. Use reloadTheme instead.`,
      );
    }

    this.prepared.set(manifest.id, candidate);
    this.publish();
    if (previous) await this.disposePrepared(previous);
  }

  async activateTheme(themeId: string): Promise<void> {
    if (themeId === BUILTIN_THEME_ID) {
      await this.activateBuiltInTheme();
      return;
    }

    const next = this.prepared.get(themeId);
    if (!next) {
      throw new ThemeRuntimeError(
        "THEME_NOT_PREPARED",
        `Theme ${themeId} is not prepared.`,
      );
    }
    if (next.owner === this.snapshot.activeOwner) return;

    const previous = this.activePreparedTheme();
    await this.runActivation(next, this.snapshot.activeThemeId);
    this.snapshot = Object.freeze({
      ...this.snapshot,
      activeThemeId: themeId,
      activeOwner: next.owner,
    });
    this.publish();
    await this.runDeactivation(previous, themeId);
  }

  async reloadTheme(manifest: ThemeManifest, definition: ThemeDefinition): Promise<void> {
    const candidate = await this.prepareCandidate(manifest, definition);
    const previous = this.prepared.get(manifest.id);
    const isActive = previous?.owner === this.snapshot.activeOwner;

    try {
      if (isActive) {
        await this.runActivation(candidate, manifest.id);
        this.prepared.set(manifest.id, candidate);
        this.snapshot = Object.freeze({
          ...this.snapshot,
          activeThemeId: manifest.id,
          activeOwner: candidate.owner,
        });
        this.publish();
        await this.runDeactivation(previous, manifest.id);
      } else {
        this.prepared.set(manifest.id, candidate);
        this.publish();
      }
    } catch (error) {
      await this.disposePrepared(candidate);
      throw error;
    }

    if (previous) await this.disposePrepared(previous);
  }

  async disposeTheme(themeId: string): Promise<void> {
    if (themeId === BUILTIN_THEME_ID) {
      throw new ThemeRuntimeError(
        "THEME_BUILTIN_PROTECTED",
        "The built-in theme cannot be disposed.",
      );
    }
    const entry = this.prepared.get(themeId);
    if (!entry) return;
    if (entry.owner === this.snapshot.activeOwner) {
      await this.activateBuiltInTheme();
    }
    this.prepared.delete(themeId);
    this.publish();
    await this.disposePrepared(entry);
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  getSnapshot(): ThemeRuntimeSnapshot {
    return this.snapshot;
  }

  private async prepareCandidate(
    manifest: ThemeManifest,
    definition: ThemeDefinition,
  ): Promise<PreparedTheme> {
    this.assertDefinition(manifest, definition);
    const owner = `${manifest.id}@${++this.ownerSequence}`;
    const services = this.options.createContextServices(manifest);
    const context: ThemeContext = {
      manifest,
      runtime: {
        appVersion: this.options.appVersion,
        themeApiVersion: THEME_API_VERSION,
        platform: this.options.platform,
        development: this.options.isDevelopmentTheme?.(manifest) ?? false,
        activeThemeId: this.snapshot.activeThemeId,
      },
      routes: this.options.routes.forOwner(owner),
      slots: this.options.slots.forOwner(owner),
      ...services,
    };

    try {
      const lifecycle = await this.runPhase(
        "setup",
        () => definition.setup(context),
        this.setupTimeoutMs,
      );
      return {
        definition,
        lifecycle: lifecycle ?? {},
        manifest,
        owner,
        context,
      };
    } catch (error) {
      this.clearOwner(owner);
      if (error instanceof ThemeRuntimeError) throw error;
      throw new ThemeRuntimeError(
        "THEME_SETUP_FAILED",
        `Theme ${manifest.id} setup failed.`,
        { cause: error },
      );
    }
  }

  private assertDefinition(manifest: ThemeManifest, definition: ThemeDefinition): void {
    const definitionApiMajor = majorVersion(definition.apiVersion);
    const hostApiMajor = majorVersion(THEME_API_VERSION);
    if (
      definition.id !== manifest.id ||
      definition.version !== manifest.version ||
      definitionApiMajor === null ||
      definitionApiMajor !== hostApiMajor
    ) {
      throw new ThemeRuntimeError(
        "THEME_DEFINITION_MISMATCH",
        "Theme Bundle metadata does not match its manifest or host API.",
      );
    }
  }

  private async runActivation(entry: PreparedTheme, previousThemeId: string): Promise<void> {
    if (!entry.lifecycle.activate) return;
    const controller = new AbortController();
    try {
      await this.runPhase(
        "activation",
        () => entry.lifecycle.activate?.({
          previousThemeId,
          signal: controller.signal,
        }),
        this.activateTimeoutMs,
        controller,
      );
    } catch (error) {
      if (error instanceof ThemeRuntimeError) throw error;
      throw new ThemeRuntimeError(
        "THEME_ACTIVATION_FAILED",
        `Theme ${entry.manifest.id} activation failed.`,
        { cause: error },
      );
    }
  }

  private async runDeactivation(
    entry: PreparedTheme | undefined,
    nextThemeId: string,
  ): Promise<void> {
    if (!entry?.lifecycle.deactivate) return;
    const controller = new AbortController();
    try {
      await this.runPhase(
        "deactivation",
        () => entry.lifecycle.deactivate?.({
          nextThemeId,
          signal: controller.signal,
        }),
        this.deactivateTimeoutMs,
        controller,
      );
    } catch (error) {
      entry.context.logger.error("Theme deactivation failed.", {
        themeId: entry.manifest.id,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  private async activateBuiltInTheme(): Promise<void> {
    if (this.snapshot.activeThemeId === BUILTIN_THEME_ID) return;
    const previous = this.activePreparedTheme();
    this.snapshot = Object.freeze({
      ...this.snapshot,
      activeThemeId: BUILTIN_THEME_ID,
      activeOwner: BUILTIN_THEME_ID,
    });
    this.publish();
    await this.runDeactivation(previous, BUILTIN_THEME_ID);
  }

  private activePreparedTheme(): PreparedTheme | undefined {
    if (this.snapshot.activeThemeId === BUILTIN_THEME_ID) return undefined;
    const entry = this.prepared.get(this.snapshot.activeThemeId);
    return entry?.owner === this.snapshot.activeOwner ? entry : undefined;
  }

  private async disposePrepared(entry: PreparedTheme): Promise<void> {
    try {
      if (entry.lifecycle.dispose) {
        await this.runPhase(
          "dispose",
          () => entry.lifecycle.dispose?.(),
          this.deactivateTimeoutMs,
        );
      }
    } catch (error) {
      entry.context.logger.error("Theme disposal failed.", {
        themeId: entry.manifest.id,
        error: error instanceof Error ? error.message : String(error),
      });
    } finally {
      this.clearOwner(entry.owner);
    }
  }

  private clearOwner(owner: string): void {
    this.options.routes.clearOwner(owner);
    this.options.slots.clearOwner(owner);
  }

  private async runPhase<T>(
    phase: ThemePhase,
    action: () => T | Promise<T>,
    timeoutMs: number,
    controller?: AbortController,
  ): Promise<T> {
    let timeout: ReturnType<typeof setTimeout> | undefined;
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeout = setTimeout(() => {
        controller?.abort();
        reject(new ThemeRuntimeError(
          `THEME_${phase.toUpperCase()}_TIMEOUT`,
          `Theme ${phase} exceeded ${timeoutMs} ms.`,
        ));
      }, timeoutMs);
    });

    try {
      return await Promise.race([
        Promise.resolve().then(action),
        timeoutPromise,
      ]);
    } finally {
      if (timeout !== undefined) clearTimeout(timeout);
    }
  }

  private publish(): void {
    this.snapshot = Object.freeze({
      ...this.snapshot,
      preparedThemeIds: Object.freeze([...this.prepared.keys()].sort()),
      revision: this.snapshot.revision + 1,
    });
    this.listeners.forEach((listener) => listener());
  }
}
