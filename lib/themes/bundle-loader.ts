"use client";

import { invoke } from "@tauri-apps/api/core";

import { acquireThemeHostBridge } from "./host-bridge";
import type {
  ThemeAssetService,
  ThemeDefinition,
  ThemeManifest,
} from "./protocol";

export interface ThemeBundleReader {
  readText(themeId: string, path: string): Promise<string>;
  readBinary(themeId: string, path: string): Promise<string>;
}

export interface ThemeBundleLoaderDependencies {
  reader: ThemeBundleReader;
  document?: Document;
  createObjectURL?(blob: Blob): string;
  revokeObjectURL?(url: string): void;
  acquireHostBridge?(): () => void;
}

export interface LoadedThemeBundle {
  readonly definition: ThemeDefinition;
  readonly assets: ThemeAssetService;
  unload(): void;
}

export class ThemeBundleError extends Error {
  constructor(readonly code: string, message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "ThemeBundleError";
  }
}

export const nativeThemeBundleReader: ThemeBundleReader = {
  readText(themeId, path) {
    return invoke<string>("theme_read_text", { themeId, path });
  },
  readBinary(themeId, path) {
    return invoke<string>("theme_read_binary", { themeId, path });
  },
};

function binaryFromBase64(value: string): ArrayBuffer {
  const binary = atob(value);
  const buffer = new ArrayBuffer(binary.length);
  const bytes = new Uint8Array(buffer);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return buffer;
}

function assetContentType(path: string): string {
  const extension = path.split(".").pop()?.toLowerCase();
  return {
    avif: "image/avif",
    gif: "image/gif",
    jpeg: "image/jpeg",
    jpg: "image/jpeg",
    png: "image/png",
    svg: "image/svg+xml",
    webp: "image/webp",
    woff: "font/woff",
    woff2: "font/woff2",
  }[extension ?? ""] ?? "application/octet-stream";
}

export async function loadThemeBundle(
  manifest: ThemeManifest,
  dependencies: ThemeBundleLoaderDependencies,
): Promise<LoadedThemeBundle> {
  const targetDocument = dependencies.document ?? document;
  const createObjectURL = dependencies.createObjectURL ?? URL.createObjectURL.bind(URL);
  const revokeObjectURL = dependencies.revokeObjectURL ?? URL.revokeObjectURL.bind(URL);
  const releaseHost = (dependencies.acquireHostBridge ?? acquireThemeHostBridge)();
  const urls = new Set<string>();
  const assetUrls = new Set<string>();
  let script: HTMLScriptElement | undefined;
  let style: HTMLLinkElement | undefined;
  let unloaded = false;

  const revoke = (url: string) => {
    if (!urls.delete(url)) return;
    assetUrls.delete(url);
    revokeObjectURL(url);
  };
  const unload = () => {
    if (unloaded) return;
    unloaded = true;
    script?.remove();
    style?.remove();
    [...urls].forEach(revoke);
    releaseHost();
  };

  try {
    if (window.__RTL_THEME_REGISTER__) {
      throw new ThemeBundleError(
        "THEME_BUNDLE_LOAD_BUSY",
        "Another Theme bundle is being registered.",
      );
    }
    const source = await dependencies.reader.readText(manifest.id, manifest.entry.script);
    const scriptUrl = createObjectURL(new Blob([source], { type: "text/javascript" }));
    urls.add(scriptUrl);
    script = targetDocument.createElement("script");
    script.src = scriptUrl;
    script.async = true;
    script.dataset.themeId = manifest.id;

    let registered: ThemeDefinition | undefined;
    const previousRegistration = window.__RTL_THEME_REGISTER__;
    const definition = await new Promise<ThemeDefinition>((resolve, reject) => {
      window.__RTL_THEME_REGISTER__ = (candidate) => {
        if (registered) {
          throw new ThemeBundleError(
            "THEME_BUNDLE_MULTIPLE_REGISTRATIONS",
            `Theme bundle ${manifest.id} registered more than once.`,
          );
        }
        if (candidate.id !== manifest.id) {
          throw new ThemeBundleError(
            "THEME_BUNDLE_ID_MISMATCH",
            `Theme bundle registered ${candidate.id} instead of ${manifest.id}.`,
          );
        }
        registered = candidate;
      };
      script!.onload = () => {
        if (!registered) {
          reject(new ThemeBundleError(
            "THEME_BUNDLE_REGISTRATION_MISSING",
            `Theme bundle ${manifest.id} did not register a definition.`,
          ));
          return;
        }
        resolve(registered);
      };
      script!.onerror = (event) => reject(new ThemeBundleError(
        "THEME_BUNDLE_LOAD_FAILED",
        `Theme bundle ${manifest.id} could not be loaded.`,
        { cause: event },
      ));
      try {
        targetDocument.head.appendChild(script!);
      } catch (error) {
        reject(error);
      }
    }).finally(() => {
      if (previousRegistration) window.__RTL_THEME_REGISTER__ = previousRegistration;
      else delete window.__RTL_THEME_REGISTER__;
    });

    if (manifest.entry.style) {
      const css = await dependencies.reader.readText(manifest.id, manifest.entry.style);
      const styleUrl = createObjectURL(new Blob([css], { type: "text/css" }));
      urls.add(styleUrl);
      style = targetDocument.createElement("link");
      style.rel = "stylesheet";
      style.href = styleUrl;
      style.dataset.themeId = manifest.id;
      targetDocument.head.appendChild(style);
    }

    const assets: ThemeAssetService = {
      async url(path) {
        if (unloaded) {
          throw new ThemeBundleError(
            "THEME_BUNDLE_UNLOADED",
            `Theme bundle ${manifest.id} is unloaded.`,
          );
        }
        const encoded = await dependencies.reader.readBinary(manifest.id, path);
        const assetUrl = createObjectURL(new Blob(
          [binaryFromBase64(encoded)],
          { type: assetContentType(path) },
        ));
        urls.add(assetUrl);
        assetUrls.add(assetUrl);
        return assetUrl;
      },
      release(url) {
        if (assetUrls.has(url)) revoke(url);
      },
    };

    return Object.freeze({ definition, assets, unload });
  } catch (error) {
    unload();
    if (error instanceof ThemeBundleError) throw error;
    throw new ThemeBundleError(
      "THEME_BUNDLE_LOAD_FAILED",
      `Theme bundle ${manifest.id} could not be loaded.`,
      { cause: error },
    );
  }
}
