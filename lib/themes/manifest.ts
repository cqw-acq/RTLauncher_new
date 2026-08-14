import {
  BUILTIN_THEME_ID,
  isCoreRouteId,
  isCoreSlotId,
  type ThemeManifest,
} from "./protocol";

export interface ThemeManifestValidationHost {
  appVersion: string;
  themeApiVersion: string;
  schemaVersion: string;
}

export type ThemeManifestValidationResult =
  | { ok: true; manifest: ThemeManifest; warnings: readonly string[] }
  | { ok: false; issues: readonly { code: string; path: string; message: string }[] };

interface Version {
  major: number;
  minor: number;
  patch: number;
}

type Issue = Extract<ThemeManifestValidationResult, { ok: false }>["issues"][number];

const THEME_ID_PATTERN = /^[a-z0-9]+(?:[.-][a-z0-9]+)+$/;
const CONTRIBUTION_ID_PATTERN = /^[a-z0-9]+(?:[._-][a-z0-9]+)+$/;
const COLOR_SCHEMES = new Set(["light", "dark"]);
const USER_OVERRIDES = new Set(["accentColor", "fontSize", "backgroundImage"]);
const ROUTE_MODES = new Set(["replace", "wrap"]);
const SLOT_MODES = new Set(["replace", "before", "after", "wrap"]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function parseVersion(value: string): Version | null {
  const match = value.trim().match(/^(\d+)\.(\d+)(?:\.(\d+))?(?:[-+].*)?$/);
  if (!match) return null;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3] ?? 0),
  };
}

function compareVersions(left: Version, right: Version): number {
  if (left.major !== right.major) return left.major - right.major;
  if (left.minor !== right.minor) return left.minor - right.minor;
  return left.patch - right.patch;
}

function satisfiesComparator(version: Version, comparator: string): boolean {
  if (/^\d+\.(?:x|\*)$/i.test(comparator)) {
    return version.major === Number(comparator.split(".")[0]);
  }
  if (/^\d+\.\d+\.(?:x|\*)$/i.test(comparator)) {
    const [major, minor] = comparator.split(".").map(Number);
    return version.major === major && version.minor === minor;
  }

  if (comparator.startsWith("^")) {
    const minimum = parseVersion(comparator.slice(1));
    if (!minimum) return false;
    const maximum = minimum.major > 0
      ? { major: minimum.major + 1, minor: 0, patch: 0 }
      : { major: 0, minor: minimum.minor + 1, patch: 0 };
    return compareVersions(version, minimum) >= 0 && compareVersions(version, maximum) < 0;
  }

  if (comparator.startsWith("~")) {
    const minimum = parseVersion(comparator.slice(1));
    if (!minimum) return false;
    const maximum = { major: minimum.major, minor: minimum.minor + 1, patch: 0 };
    return compareVersions(version, minimum) >= 0 && compareVersions(version, maximum) < 0;
  }

  const match = comparator.match(/^(>=|<=|>|<|=)?(.+)$/);
  if (!match) return false;
  const expected = parseVersion(match[2]);
  if (!expected) return false;
  const comparison = compareVersions(version, expected);
  switch (match[1] ?? "=") {
    case ">=": return comparison >= 0;
    case "<=": return comparison <= 0;
    case ">": return comparison > 0;
    case "<": return comparison < 0;
    default: return comparison === 0;
  }
}

function satisfiesVersionRange(versionValue: string, range: string): boolean {
  const version = parseVersion(versionValue);
  if (!version || !isNonEmptyString(range)) return false;
  return range.split("||").some((alternative) => {
    const comparators = alternative.trim().split(/\s+/).filter(Boolean);
    return comparators.length > 0 && comparators.every((item) =>
      satisfiesComparator(version, item),
    );
  });
}

export function isSafeThemePath(path: string): boolean {
  if (!path || path.startsWith("/") || path.includes("\\") || path.includes("//")) {
    return false;
  }
  if (/^[a-zA-Z]:/.test(path)) return false;
  const segments = path.split("/");
  return segments.every((segment) => segment.length > 0 && segment !== "." && segment !== "..");
}

function pushIssue(
  issues: Issue[],
  code: string,
  path: string,
  message: string,
): void {
  issues.push({ code, path, message });
}

function requireString(
  value: unknown,
  path: string,
  issues: Issue[],
): value is string {
  if (isNonEmptyString(value)) return true;
  pushIssue(issues, "THEME_MANIFEST_INVALID", path, `${path} must be a non-empty string.`);
  return false;
}

function validatePackagePath(
  value: unknown,
  path: string,
  issues: Issue[],
  required = false,
): void {
  if (value === undefined && !required) return;
  if (!isNonEmptyString(value) || !isSafeThemePath(value)) {
    pushIssue(issues, "THEME_PATH_INVALID", path, `${path} must be a safe package-relative path.`);
  }
}

function validateContributionId(
  value: unknown,
  path: string,
  seen: Set<string>,
  issues: Issue[],
): void {
  if (!isNonEmptyString(value) || !CONTRIBUTION_ID_PATTERN.test(value)) {
    pushIssue(issues, "THEME_CONTRIBUTION_INVALID", path, `${path} must be a namespaced identifier.`);
    return;
  }
  if (seen.has(value)) {
    pushIssue(issues, "THEME_CONTRIBUTION_DUPLICATE", path, `Contribution ID ${value} is duplicated.`);
    return;
  }
  seen.add(value);
}

function validateContributions(value: unknown, issues: Issue[]): void {
  if (value === undefined) return;
  if (!isRecord(value)) {
    pushIssue(issues, "THEME_MANIFEST_INVALID", "contributes", "contributes must be an object.");
    return;
  }

  const seen = new Set<string>();
  if (value.routes !== undefined) {
    if (!Array.isArray(value.routes)) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "contributes.routes", "routes must be an array.");
    } else {
      value.routes.forEach((route, index) => {
        const path = `contributes.routes.${index}`;
        if (!isRecord(route)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", path, "Route contribution must be an object.");
          return;
        }
        validateContributionId(route.id, `${path}.id`, seen, issues);
        if (!isNonEmptyString(route.mode) || !ROUTE_MODES.has(route.mode)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", `${path}.mode`, "Route mode must be replace or wrap.");
        }
        const target = isNonEmptyString(route.target) ? route.target : null;
        const routePath = isNonEmptyString(route.path) ? route.path : null;
        if (Boolean(target) === Boolean(routePath)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", path, "A route needs either target or path.");
        } else if (target && !isCoreRouteId(target)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", `${path}.target`, "Unknown core route ID.");
        } else if (routePath && !routePath.startsWith("/theme/")) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", `${path}.path`, "Theme pages must use /theme/ paths.");
        }
      });
    }
  }

  if (value.slots !== undefined) {
    if (!Array.isArray(value.slots)) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "contributes.slots", "slots must be an array.");
    } else {
      value.slots.forEach((slot, index) => {
        const path = `contributes.slots.${index}`;
        if (!isRecord(slot)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", path, "Slot contribution must be an object.");
          return;
        }
        validateContributionId(slot.id, `${path}.id`, seen, issues);
        if (!isNonEmptyString(slot.target) || !isCoreSlotId(slot.target)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", `${path}.target`, "Unknown core slot ID.");
        }
        if (!isNonEmptyString(slot.mode) || !SLOT_MODES.has(slot.mode)) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", `${path}.mode`, "Unknown slot mode.");
        }
        if (slot.order !== undefined && (!Number.isInteger(slot.order) || typeof slot.order !== "number")) {
          pushIssue(issues, "THEME_CONTRIBUTION_INVALID", `${path}.order`, "Slot order must be an integer.");
        }
      });
    }
  }

  if (value.settings !== undefined) {
    if (!isRecord(value.settings)) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "contributes.settings", "settings must be an object.");
    } else {
      validatePackagePath(value.settings.schema, "contributes.settings.schema", issues, true);
      validatePackagePath(value.settings.defaults, "contributes.settings.defaults", issues);
    }
  }
}

export function validateThemeManifest(
  value: unknown,
  host: ThemeManifestValidationHost,
): ThemeManifestValidationResult {
  const issues: Issue[] = [];
  if (!isRecord(value)) {
    return {
      ok: false,
      issues: [{ code: "THEME_MANIFEST_INVALID", path: "", message: "Manifest must be an object." }],
    };
  }

  requireString(value.schemaVersion, "schemaVersion", issues);
  if (
    isNonEmptyString(value.schemaVersion) &&
    parseVersion(value.schemaVersion)?.major !== parseVersion(host.schemaVersion)?.major
  ) {
    pushIssue(issues, "THEME_SCHEMA_INCOMPATIBLE", "schemaVersion", "Theme schema major version is not supported.");
  }

  if (
    !isNonEmptyString(value.id) ||
    !THEME_ID_PATTERN.test(value.id) ||
    value.id === BUILTIN_THEME_ID ||
    value.id.startsWith("builtin.")
  ) {
    pushIssue(issues, "THEME_ID_INVALID", "id", "Theme ID must be a lowercase reverse-domain identifier.");
  }
  requireString(value.name, "name", issues);
  if (requireString(value.version, "version", issues) && !parseVersion(value.version)) {
    pushIssue(issues, "THEME_MANIFEST_INVALID", "version", "Theme version must use SemVer.");
  }

  if (!isRecord(value.author)) {
    pushIssue(issues, "THEME_MANIFEST_INVALID", "author", "author must be an object.");
  } else {
    requireString(value.author.name, "author.name", issues);
    if (value.author.url !== undefined) requireString(value.author.url, "author.url", issues);
  }

  if (!isRecord(value.engines)) {
    pushIssue(issues, "THEME_MANIFEST_INVALID", "engines", "engines must be an object.");
  } else {
    if (requireString(value.engines.rtlauncher, "engines.rtlauncher", issues)) {
      if (!satisfiesVersionRange(host.appVersion, value.engines.rtlauncher)) {
        pushIssue(issues, "THEME_APP_INCOMPATIBLE", "engines.rtlauncher", "RTLauncher version is outside the supported range.");
      }
    }
    if (requireString(value.engines.themeApi, "engines.themeApi", issues)) {
      if (!satisfiesVersionRange(host.themeApiVersion, value.engines.themeApi)) {
        pushIssue(issues, "THEME_API_INCOMPATIBLE", "engines.themeApi", "Theme API version is outside the supported range.");
      }
    }
  }

  if (!isRecord(value.entry)) {
    pushIssue(issues, "THEME_MANIFEST_INVALID", "entry", "entry must be an object.");
  } else {
    validatePackagePath(value.entry.script, "entry.script", issues, true);
    validatePackagePath(value.entry.style, "entry.style", issues);
  }

  validatePackagePath(value.icon, "icon", issues);
  if (value.previews !== undefined) {
    if (!Array.isArray(value.previews)) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "previews", "previews must be an array.");
    } else {
      value.previews.forEach((preview, index) =>
        validatePackagePath(preview, `previews.${index}`, issues, true),
      );
    }
  }

  if (!isRecord(value.supports) || !Array.isArray(value.supports.colorSchemes)) {
    pushIssue(issues, "THEME_MANIFEST_INVALID", "supports.colorSchemes", "colorSchemes must be an array.");
  } else {
    if (value.supports.colorSchemes.length === 0) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "supports.colorSchemes", "At least one color scheme is required.");
    }
    value.supports.colorSchemes.forEach((scheme, index) => {
      if (!isNonEmptyString(scheme) || !COLOR_SCHEMES.has(scheme)) {
        pushIssue(issues, "THEME_MANIFEST_INVALID", `supports.colorSchemes.${index}`, "Unknown color scheme.");
      }
    });
    if (value.supports.userOverrides !== undefined) {
      if (!Array.isArray(value.supports.userOverrides)) {
        pushIssue(issues, "THEME_MANIFEST_INVALID", "supports.userOverrides", "userOverrides must be an array.");
      } else {
        value.supports.userOverrides.forEach((override, index) => {
          if (!isNonEmptyString(override) || !USER_OVERRIDES.has(override)) {
            pushIssue(issues, "THEME_MANIFEST_INVALID", `supports.userOverrides.${index}`, "Unknown user override.");
          }
        });
      }
    }
  }

  validateContributions(value.contributes, issues);

  if (value.integrity !== undefined) {
    if (!isRecord(value.integrity) || value.integrity.algorithm !== "sha256" || !isRecord(value.integrity.files)) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "integrity", "integrity must declare sha256 file hashes.");
    } else {
      Object.entries(value.integrity.files).forEach(([path, hash]) => {
        if (!isSafeThemePath(path) || !isNonEmptyString(hash)) {
          pushIssue(issues, "THEME_PATH_INVALID", `integrity.files.${path}`, "Integrity entries need a safe path and hash.");
        }
      });
    }
  }

  if (value.extensions !== undefined) {
    if (!isRecord(value.extensions)) {
      pushIssue(issues, "THEME_MANIFEST_INVALID", "extensions", "extensions must be an object.");
    } else {
      Object.keys(value.extensions).forEach((key) => {
        if (!key.includes(":")) {
          pushIssue(issues, "THEME_MANIFEST_INVALID", `extensions.${key}`, "Extension keys must have a namespace.");
        }
      });
    }
  }

  if (issues.length > 0) return { ok: false, issues };
  return { ok: true, manifest: value as unknown as ThemeManifest, warnings: [] };
}
