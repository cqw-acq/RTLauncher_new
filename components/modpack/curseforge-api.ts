export interface CurseforgeApiFile {
  id: number;
  displayName?: string;
  fileName?: string;
  releaseType?: number;
  fileDate?: string;
  gameVersions?: string[];
  downloadUrl?: string | null;
  dependencies?: Array<{
    modId: number;
    /** CurseForge: 3 = RequiredDependency。 */
    relationType: number;
  }>;
}

/** CurseForge REST API 的 ModLoaderType 枚举。 */
export const CURSEFORGE_MOD_LOADER_TYPES: Readonly<Record<string, number>> = {
  forge: 1,
  liteloader: 3,
  fabric: 4,
  quilt: 5,
  neoforge: 6,
};

export function buildCurseforgeFilesUrl(
  projectId: number,
  gameVersion?: string,
  modLoaderType?: number,
  index = 0,
): string {
  const params = new URLSearchParams({ pageSize: "50", index: String(index) });
  if (gameVersion) params.set("gameVersion", gameVersion);
  if (modLoaderType !== undefined) {
    params.set("modLoaderType", String(modLoaderType));
  }
  return `https://api.curseforge.com/v1/mods/${projectId}/files?${params.toString()}`;
}

export function filterCurseforgeFilesByGameVersion(
  files: CurseforgeApiFile[],
  gameVersion?: string,
): CurseforgeApiFile[] {
  if (!gameVersion) return files;
  return files.filter(
    (file) =>
      Array.isArray(file.gameVersions) && file.gameVersions.includes(gameVersion),
  );
}
