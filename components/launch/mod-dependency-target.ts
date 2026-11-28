import type { InstanceData } from "@/types";

/** Loader keywords the backend understands; "neoforge" must be tested before "forge". */
const KNOWN_LOADERS = ["neoforge", "fabric", "quilt", "liteloader", "ornithe", "forge"] as const;

export interface DependencyTargetConfig {
  minecraftVersion?: string;
  versionName?: string;
  loadName?: string;
}

export interface DependencyTarget {
  mcVersion: string;
  loader: string;
}

/** Finds a known loader keyword in free text such as "NeoForge" or "e2e-fabric-1.21.1". */
export function detectLoaderKeyword(value: string | null | undefined): string | undefined {
  const lower = (value ?? "").toLowerCase();
  return KNOWN_LOADERS.find((loader) => lower.includes(loader));
}

/**
 * Minecraft version and loader used to look up and install missing mod dependencies.
 *
 * The scanned instance (read from its version JSON) is authoritative. The launch
 * config is only a fallback: `loadName` is a folder name, and stripping it to
 * letters turned names like "e2e-fabric-1.21.1" into "eefabric", which matches
 * no loader once the backend requires the loader to match exactly.
 */
export function resolveDependencyTarget(
  instance: Pick<InstanceData, "minecraft_version" | "loader"> | null | undefined,
  config: DependencyTargetConfig,
): DependencyTarget {
  const mcVersion =
    instance?.minecraft_version?.trim() ||
    config.minecraftVersion?.trim() ||
    config.versionName?.trim() ||
    "1.20.1";
  const loader =
    detectLoaderKeyword(instance?.loader) ??
    detectLoaderKeyword(config.loadName) ??
    detectLoaderKeyword(config.versionName) ??
    "forge";
  return { mcVersion, loader };
}
