"use client";

import { useState, useEffect, useMemo, useCallback } from "react";
import { ArrowLeft, ChevronDown, Shield, FlaskConical, Loader2, Download, CheckCircle2, XCircle, Package, ExternalLink } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { invoke } from "@tauri-apps/api/core";
import { useDownloadManager } from "@/components/download/download-provider";
import { useSettings } from "@/components/settings/settings-provider";
import { useRouter } from "next/navigation";

const openExternalUrl = async (url: string) => {
  try {
    await invoke("open_external", { url });
  } catch (err) {
    console.error("Failed to open URL:", err);
    if (typeof window !== "undefined") {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  }
};

interface ModFiles {
  [mcVersion: string]: Array<[string[], string]>;
}

interface LiveModDetail {
  slug: string;
  title: string;
  description?: string;
  body?: string;
  iconUrl?: string;
  projectType?: string;
  downloads?: number;
  categories?: string[];
  gameVersions?: string[];
  latestVersions?: string[];
  updated?: string;
  author?: string;
  source: 'modrinth' | 'curseforge' | 'both';
  sources: {
    modrinth: { ok: boolean; url?: string; error?: string };
    curseforge: { ok: boolean; url?: string; error?: string };
  };
  modrinthUrl?: string;
  curseforgeUrl?: string;
  mcmodUrl?: string;
  classId?: number;
}

interface ParsedFile {
  tags: string[];
  cleanTags: string[];
  url: string;
  isRelease: boolean;
  hasForge: boolean;
  hasFabric: boolean;
  hasNeoForge: boolean;
  hasQuilt: boolean;
  hasLiteLoader: boolean;
  hasOrnithe: boolean;
  loaderLabel: string;
  versionLabel: string;
  serverLabel: string;
}

interface ResolvedModDependency {
  projectId: string;
  projectSlug: string;
  projectName: string;
  downloadUrl: string;
}

function decodeUriSafe(s: string): string {
  try {
    return decodeURIComponent(s);
  } catch {
    return s;
  }
}

/** CurseForge classId -> 项目类型（用于推断非 Modrinth 项目的类型） */
function classIdToProjectType(classId: number): string {
  switch (classId) {
    case 6: return "mod";
    case 12: return "resourcepack";
    case 6552: return "shader";
    case 6945: return "datapack";
    case 6949: return "datapack";
    case 17: return "world";
    case 4471: return "modpack";
    case 4473: return "modpack";
    default: return "mod";
  }
}

function cleanFileName(name: string): string {
  if (!name) return name;
  let s = name;
  s = decodeUriSafe(s);
  s = s.replace(/\+/g, " ");
  s = s.replace(/_{2,}/g, " ").replace(/\.{2,}/g, ".");
  s = s.replace(/\s{2,}/g, " ").trim();
  return s;
}

function formatDownloads(n?: number): string {
  if (n === undefined || n === null) return "";
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, "") + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, "") + "K";
  return String(n);
}

function translateProjectType(pt?: string): string {
  if (!pt) return "Unknown";
  const lower = pt.toLowerCase();
  const map: Record<string, string> = {
    mod: "Mod",
    "minecraft mod": "Mod",
    modpack: "Modpack",
    "mod pack": "Modpack",
    resourcepack: "Resource Pack",
    "resource pack": "Resource Pack",
    "texture pack": "Resource Pack",
    shader: "Shader",
    shaders: "Shader",
    "shader pack": "Shader",
    datapack: "Data Pack",
    "data pack": "Data Pack",
    world: "World",
    worlds: "World",
  };
  return map[lower] ?? pt;
}

function translateCategory(cat: string): string {
  const map: Record<string, string> = {
    "forge": "Forge",
    "fabric": "Fabric",
    "neoforge": "NeoForge",
    "quilt": "Quilt",
    "vanilla": "Vanilla",
    "utility": "Utility",
    "storage": "Storage",
    "decoration": "Decoration",
    "library": "Library",
    "library / api": "Library API",
    "api and library": "Library API",
    "magic": "Magic",
    "technology": "Technology",
    "tech": "Tech",
    "adventure": "Adventure",
    "adventure and rpg": "Adventure RPG",
    "rpg": "RPG",
    "world gen": "World Gen",
    "world generation": "World Generation",
    "dungeons and dimensions": "Dungeons & Dimensions",
    "dungeons": "Dungeons",
    "dimensions": "Dimensions",
    "entities": "Entities",
    "mobs": "Mobs",
    "food": "Food",
    "farming": "Farming",
    "energy": "Energy",
    "redstone": "Redstone",
    "automation": "Automation",
    "transport": "Transport",
    "buildcraft": "Building",
    "combat": "Combat",
    "armor, tools, and weapons": "Armor, Tools & Weapons",
    "armor, tools & weapons": "Armor, Tools & Weapons",
    "performance": "Performance",
    "optimization": "Optimization",
    "qol": "QoL",
    "quality of life": "Quality of Life",
    "information": "Information",
    "tweaks": "Tweaks",
    "cosmetic": "Cosmetic",
    "environmental": "Environmental",
    "biomes": "Biomes",
    "structures": "Structures",
    "miscellaneous": "Miscellaneous",
    "misc": "Misc",
  };
  const lower = cat.toLowerCase();
  return map[lower] ?? cat;
}

function formatDateShort(iso?: string): string {
  if (!iso) return "";
  try {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return "";
    const now = Date.now();
    const diffMs = now - d.getTime();
    const day = 24 * 60 * 60 * 1000;
    if (diffMs < day) return "Today";
    if (diffMs < 7 * day) return Math.round(diffMs / day) + " days ago";
    if (diffMs < 30 * day) return Math.round(diffMs / (7 * day)) + " weeks ago";
    if (diffMs < 365 * day) return Math.round(diffMs / (30 * day)) + " months ago";
    return Math.round(diffMs / (365 * day)) + " years ago";
  } catch {
    return "";
  }
}

function isReleaseVersion(tags: string[]): boolean {
  const releaseKeywords = ["release", "Release", "RELEASE", "stable", "Stable", "final", "Final"];
  const betaKeywords = ["beta", "Beta", "test", "alpha", "Alpha", "snapshot", "Snapshot", "SNAPSHOT", "experimental", "Experimental", "dev", "DEV", "Dev"];

  for (const tag of tags) {
    const lowerTag = tag.toLowerCase();
    if (betaKeywords.some(k => lowerTag.includes(k.toLowerCase()))) {
      return false;
    }
  }
  for (const tag of tags) {
    const lowerTag = tag.toLowerCase();
    if (releaseKeywords.some(k => lowerTag.includes(k.toLowerCase()))) {
      return true;
    }
  }
  return true;
}

function extractLoaderInfo(tags: string[]) {
  let hasForge = false;
  let hasFabric = false;
  let hasNeoForge = false;
  let hasQuilt = false;
  let hasLiteLoader = false;
  let hasOrnithe = false;
  let loaderLabel = "";
  let isServer = false;
  let isClient = false;

  for (const tag of tags) {
    const lower = tag.toLowerCase();
    if (lower === "server" || lower === "server-1" || lower.includes("server")) {
      isServer = true;
    } else if (lower.includes("neoforge") || lower.includes("neo")) {
      hasNeoForge = true;
      if (!loaderLabel) loaderLabel = "NeoForge";
    } else if (lower.includes("forge") && !lower.includes("neo")) {
      hasForge = true;
      if (!loaderLabel) loaderLabel = "Forge";
    } else if (lower.includes("fabric")) {
      hasFabric = true;
      if (!loaderLabel) loaderLabel = "Fabric";
    } else if (lower.includes("quilt")) {
      hasQuilt = true;
      if (!loaderLabel) loaderLabel = "Quilt";
    } else if (lower.includes("liteloader") || lower.includes("lite") || lower.includes("litemod")) {
      hasLiteLoader = true;
      if (!loaderLabel) loaderLabel = "LiteLoader";
    } else if (lower.includes("ornithe")) {
      hasOrnithe = true;
      if (!loaderLabel) loaderLabel = "Ornithe";
    } else if (lower === "client") {
      isClient = true;
    }
  }

  if (!loaderLabel) loaderLabel = "Universal";

  let serverLabel = "";
  if (isServer && isClient) {
    serverLabel = "Server + Client";
  } else if (isServer) {
    serverLabel = "Server";
  } else if (isClient) {
    serverLabel = "Client";
  }

  return { hasForge, hasFabric, hasNeoForge, hasQuilt, hasLiteLoader, hasOrnithe, loaderLabel, serverLabel };
}

function extractVersionLabel(url: string, tags: string[]): string {
  // Exclude pure MC version formats: e.g., 1.21, 1.21.1, 1.20.4, etc.
  const isMcVersion = (s: string): boolean => {
    const trimmed = s.trim();
    // Pure numeric dot format: 1.x or 1.x.x
    return /^\d+\.\d+(\.\d+)?$/.test(trimmed);
  };

  // 1) First find real mod version number from tags (not pure MC version, with version characteristics)
  for (const tag of tags) {
    const clean = cleanFileName(tag);
    if (!clean || clean.length > 40) continue;
    // Accept: v1.2.3 / 2.0.1+mc1.21 / modname-1.2.3, etc.
    // Reject: pure MC version (e.g., 1.21, 1.20.4)
    if (/v?\d+\.\d+/.test(clean) && !isMcVersion(clean)) {
      return clean;
    }
  }

  // 2) Extract from URL filename
  try {
    const parts = url.split("/");
    const fileName = parts[parts.length - 1].split("?")[0];
    if (fileName.length > 0) {
      const simpleName = cleanFileName(fileName).replace(/\.[^.]+$/, "");
      if (simpleName.length > 0) {
        return simpleName.length > 60 ? simpleName.substring(0, 57) + "..." : simpleName;
      }
    }
  } catch {
    // ignore
  }

  // 3) Fallback: find first non-pure MC version numeric tag from tags
  for (const tag of tags) {
    const clean = cleanFileName(tag);
    if (!clean) continue;
    if (!isMcVersion(clean) && clean.length < 40) {
      return clean;
    }
  }

  return "Unknown Version";
}

function cleanTags(tags: string[], mcVersion: string, loaderLabel: string, serverLabel: string): string[] {
  const skip = new Set<string>();
  skip.add(mcVersion.toLowerCase());
  skip.add("java");
  const loaders = ["forge", "fabric", "neoforge", "neo", "quilt", loaderLabel.toLowerCase()];
  for (const l of loaders) skip.add(l);
  const releases = ["release", "beta", "alpha", "snapshot", "stable", "experimental"];
  for (const r of releases) skip.add(r);
  skip.add("server");
  skip.add("client");
  const result: string[] = [];
  const seen = new Set<string>();
  for (const raw of tags) {
    const t = cleanFileName(raw);
    const lower = t.toLowerCase();
    if (seen.has(lower)) continue;
    if (!t || t.length === 0) continue;
    if (skip.has(lower)) continue;
    if (lower.startsWith("java") || /^java\s*\d+/i.test(t)) continue;
    if (/^\d+\.\d+(\.\d+)?$/.test(t.trim())) continue;
    if (t.length > 40) continue;
    result.push(t);
    seen.add(lower);
  }
  return result;
}

function compareMinecraftVersionDescending(a: string, b: string): number {
  const parts = (version: string) => {
    const match = version.match(/^(\d+(?:\.\d+)+)/);
    return match ? match[1].split(".").map(Number) : null;
  };
  const aParts = parts(a);
  const bParts = parts(b);
  if (aParts && bParts) {
    const length = Math.max(aParts.length, bParts.length);
    for (let index = 0; index < length; index += 1) {
      const difference = (bParts[index] ?? 0) - (aParts[index] ?? 0);
      if (difference !== 0) return difference;
    }
  }
  return b.localeCompare(a, undefined, { numeric: true });
}

export default function ModDetailContent({ modId }: { modId: string }) {
  const router = useRouter();
  const [liveInfo, setLiveInfo] = useState<LiveModDetail | null>(null);
  const [liveError, setLiveError] = useState<string | null>(null);
  const [modFiles, setModFiles] = useState<ModFiles | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [expandedVersions, setExpandedVersions] = useState<Set<string>>(new Set());
  const [downloadingUrlToTaskId, setDownloadingUrlToTaskId] = useState<Map<string, number>>(new Map());
  const [filesError, setFilesError] = useState<string | null>(null);
  const [dataSource, setDataSource] = useState<string | null>(null);
  const { startModDownload, startResourceDownload, tasks } = useDownloadManager();
  const { settings } = useSettings();

  const urlParams = typeof window !== "undefined" ? new URLSearchParams(window.location.search) : null;
  const forceType = urlParams?.get("forceType") || null;
  const returnTo = urlParams?.get("returnTo") || null;

  // 根据 URL 获取下载状态（用 taskId 精确匹配）
  const getDownloadStatus = (url: string) => {
    const taskId = downloadingUrlToTaskId.get(url);
    if (taskId !== undefined) {
      if (taskId === -1) {
        // 正在等待后端返回 taskId，显示"下载中"
        return "downloading";
      }
      const task = tasks.find(t => t.taskId === taskId);
      if (task) {
        if (task.status === "success" || task.status === "warning") return "success";
        if (task.status === "error") return "error";
        if (task.status === "downloading" || task.status === "queued") return "downloading";
        if (task.status === "cancelled") return "idle";
      } else {
        // task 可能已被清除（clearFinished / removeTask），视为已完成
        return "success";
      }
    }
    // 降级：通过 label 模糊匹配
    const fallbackTask = tasks.find(t => {
      return t.label.includes(modId) || t.label.includes(liveInfo?.slug || modId);
    });
    if (fallbackTask?.status === "success" || fallbackTask?.status === "warning") return "success";
    return "idle";
  };

  const parsedFiles = useMemo(() => {
    if (!modFiles) return new Map<string, ParsedFile[]>();

    const result = new Map<string, ParsedFile[]>();
    for (const [mcVersion, files] of Object.entries(modFiles).sort(([a], [b]) => compareMinecraftVersionDescending(a, b))) {
      const parsed: ParsedFile[] = files.map(([tags, url]) => {
        const isRelease = isReleaseVersion(tags);
        const loaderInfo = extractLoaderInfo(tags);
        const versionLabel = extractVersionLabel(url, tags);
        const clean = cleanTags(tags, mcVersion, loaderInfo.loaderLabel, loaderInfo.serverLabel);
        return {
          tags,
          cleanTags: clean,
          url,
          isRelease,
          ...loaderInfo,
          versionLabel,
        };
      });
      parsed.sort((a, b) => {
        if (a.isRelease !== b.isRelease) return a.isRelease ? -1 : 1;
        return 0;
      });
      result.set(mcVersion, parsed);
    }
    return result;
  }, [modFiles]);

  useEffect(() => {
    loadModData();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [modId]);

  const loadLiveInfo = async () => {
    try {
      setLiveError(null);

      // Query Modrinth and CurseForge in parallel to get complete project information
      const mrPromise = invoke<string>('get_modrinth_project', { slug: modId })
        .then((result) => JSON.parse(result))
        .catch((error) => {
          console.warn('Modrinth project lookup failed:', error);
          return null;
        });

      const cfUrl = `https://api.curseforge.com/v1/mods/search?slug=${encodeURIComponent(modId)}&gameId=432`;
      const cfPromise = fetch(cfUrl, {
        headers: {
          'x-api-key': '$2a$10$VTAFCxje5a1Jkqv0aGWjQ.fULedAEPctDqppOkNMRVv.edVnG7KQ6',
          Accept: 'application/json',
          'User-Agent': 'RTLauncher',
        },
      })
        .then((r) => (r.ok ? r.json() : null))
        .catch(() => null);

      const [mrData, cfData, mcmodData] = await Promise.all([
        mrPromise,
        cfPromise,
        invoke<string>("search_moddata", { keyword: modId }).then(result => {
          try {
            const parsed = JSON.parse(result) as { slug: string; chinese_name: string; mcmod_id?: number }[];
            return parsed.find(r => r.slug.toLowerCase() === modId.toLowerCase()) || null;
          } catch {
            return null;
          }
        }).catch(() => null)
      ]);

      let mrTitle = '';
      let mrDescription = '';
      let mrDownloads: number | undefined;
      let mrIconUrl: string | undefined;
      let mrAuthor: string | undefined;
      let mrUpdated: string | undefined;
      let mrCategories: string[] = [];
      let mrLoaders: string[] = [];
      let mrProjectType: string | undefined;
      let mrOk = false;

      if (mrData && typeof mrData === 'object') {
        mrOk = true;
        mrTitle = mrData.title || mrData.name || modId;
        mrDescription = mrData.description || '';
        mrDownloads = typeof mrData.downloads === 'number' ? mrData.downloads : undefined;
        mrIconUrl = mrData.icon_url || undefined;
        mrAuthor = mrData.team
          ? undefined
          : mrData.author || undefined;
        mrUpdated = mrData.updated || mrData.date_modified || mrData.published || undefined;
        mrCategories = Array.isArray(mrData.categories) ? mrData.categories : [];
        mrLoaders = Array.isArray(mrData.loaders) ? mrData.loaders : [];
        mrProjectType = mrData.project_type || 'mod';

        // Modrinth marks datapack / world project_type as "mod".
        // Need to further determine type through loaders / categories.
        const loadersLower = mrLoaders.map((l) => (l || '').toLowerCase());
        const categoriesLower = mrCategories.map((c) => (c || '').toLowerCase());

        if (
          loadersLower.includes('datapack') ||
          categoriesLower.includes('datapack') ||
          categoriesLower.includes('data pack') ||
          categoriesLower.includes('data-pack')
        ) {
          mrProjectType = 'datapack';
        } else if (
          loadersLower.includes('minecraft') ||
          categoriesLower.includes('world') ||
          categoriesLower.includes('map')
        ) {
          // Save/Map type (determined by "world", "map" tags in categories)
          mrProjectType = 'world';
        } else if (
          categoriesLower.includes('modpack') ||
          categoriesLower.includes('mod pack') ||
          categoriesLower.includes('modpacks')
        ) {
          // Modpack (determined by "modpack" tag in categories)
          mrProjectType = 'modpack';
        }
      }

      let cfTitle = '';
      let cfDescription = '';
      let cfDownloads: number | undefined;
      let cfIconUrl: string | undefined;
      let cfAuthor: string | undefined;
      let cfUpdated: string | undefined;
      let cfOk = false;
      let cfId: number | undefined;
      let cfClassId: number | undefined;

      if (
        cfData &&
        typeof cfData === 'object' &&
        Array.isArray(cfData.data) &&
        cfData.data.length > 0
      ) {
        cfOk = true;
        const exact =
          cfData.data.find(
            (d: any) => (d.slug || '').toLowerCase() === modId.toLowerCase()
          ) || cfData.data[0];
        cfTitle = exact.name || '';
        cfDescription = exact.summary || '';
        cfDownloads = typeof exact.downloadCount === 'number' ? exact.downloadCount : undefined;
        cfIconUrl = exact.logo?.thumbnailUrl || exact.logo?.url || undefined;
        cfAuthor = exact.authors?.[0]?.name || undefined;
        cfUpdated = exact.dateModified || exact.dateReleased || undefined;
        cfId = typeof exact.id === 'number' ? exact.id : undefined;
        cfClassId = typeof exact.classId === 'number' ? exact.classId : undefined;
      }

      const title = mrTitle || cfTitle || modId;
      const description = mrDescription || cfDescription || '';
      const downloads = mrDownloads ?? cfDownloads;
      const iconUrl = mrIconUrl || cfIconUrl;
      const author = mrAuthor || cfAuthor;
      const updated = mrUpdated || cfUpdated;
      const categories = mrCategories.length > 0 ? mrCategories : [];
      // Project type selection logic:
      // 0. If URL has forceType param (e.g., from mcmod search), always use it (highest priority)
      // 1. If Modrinth's project_type is not "mod" (clearly resourcepack/shader/modpack), prioritize it
      // 2. Otherwise if CurseForge has classId, use type inferred from classId
      // 3. Otherwise use Modrinth's project_type
      // 4. Finally fallback to "mod"
      // Reason: Modrinth marks datapack/world project_type as "mod",
      // need to further identify through CurseForge's classId or loaders/categories
      let projectType: string = 'mod';
      if (forceType) {
        projectType = forceType;
      } else {
        const cfProjectType = cfClassId ? classIdToProjectType(cfClassId) : undefined;
        if (mrProjectType && mrProjectType !== 'mod') {
          projectType = mrProjectType;
        } else if (cfProjectType) {
          projectType = cfProjectType;
        } else if (mrProjectType) {
          projectType = mrProjectType;
        }
      }

      const source: 'modrinth' | 'curseforge' | 'both' =
        mrOk && cfOk ? 'both' : mrOk ? 'modrinth' : cfOk ? 'curseforge' : 'both';

      const modrinthUrl = mrOk ? `https://modrinth.com/${projectType}/${modId}` : undefined;
      // Infer correct CurseForge URL path based on projectType
      const cfPath = (() => {
        const pt = projectType.toLowerCase();
        if (pt.includes("modpack")) return "modpacks";
        if (pt.includes("resourcepack") || pt.includes("texture")) return "texture-packs";
        if (pt.includes("shader")) return "shaders";
        if (pt.includes("datapack")) return "data-packs";
        if (pt.includes("world")) return "worlds";
        return "mc-mods";
      })();
      const curseforgeUrl = cfOk
        ? `https://www.curseforge.com/minecraft/${cfPath}/${modId}`
        : undefined;

      const mcmodUrl = mcmodData?.mcmod_id ? `https://www.mcmod.cn/class/${mcmodData.mcmod_id}.html` : undefined;

      setLiveInfo({
        slug: modId,
        title,
        description,
        iconUrl,
        projectType,
        downloads,
        categories,
        updated,
        author,
        source,
        sources: {
          modrinth: { ok: mrOk, url: modrinthUrl },
          curseforge: { ok: cfOk, url: curseforgeUrl },
        },
        modrinthUrl,
        curseforgeUrl,
        mcmodUrl,
        classId: cfId,
      });
    } catch (err) {
      console.error('Failed to get project online information:', err);
      setLiveError(String(err));
      const fallbackType = forceType || "mod";
      const cfPathFallback = (() => {
        const pt = fallbackType.toLowerCase();
        if (pt.includes("modpack")) return "modpacks";
        if (pt.includes("resourcepack") || pt.includes("texture")) return "texture-packs";
        if (pt.includes("shader")) return "shaders";
        if (pt.includes("datapack")) return "data-packs";
        if (pt.includes("world")) return "worlds";
        return "mc-mods";
      })();
      // At least keep basic URL information after failure
      setLiveInfo({
        slug: modId,
        title: modId,
        projectType: fallbackType,
        source: 'both',
        sources: {
          modrinth: { ok: true, url: `https://modrinth.com/${fallbackType}/${modId}` },
          curseforge: { ok: true, url: `https://www.curseforge.com/minecraft/${cfPathFallback}/${modId}` },
        },
        modrinthUrl: `https://modrinth.com/${fallbackType}/${modId}`,
        curseforgeUrl: `https://www.curseforge.com/minecraft/${cfPathFallback}/${modId}`,
      });
    }
  };

  const loadModData = async () => {
    setLoading(true);

    const infoPromise = loadLiveInfo();

    const filesPromise = (async () => {
      try {
        setLoadingFiles(true);
        setFilesError(null);

        const cfPromise = invoke<string>("get_mod_files_by_slug", { slug: modId })
          .then((r) => ({ ok: true as const, source: "CurseForge", data: r }))
          .catch((e) => ({ ok: false as const, source: "CurseForge", error: String(e) }));
        const mrPromise = invoke<string>("get_modrinth_mod_files", { slug: modId })
          .then((r) => ({ ok: true as const, source: "Modrinth", data: r }))
          .catch((e) => ({ ok: false as const, source: "Modrinth", error: String(e) }));

        const [cfResult, mrResult] = await Promise.all([cfPromise, mrPromise]);

        let merged: ModFiles = {};
        let firstNonEmpty: string | null = null;

        // 从 URL 中提取文件名（去除查询参数和路径）
        const extractFilename = (url: string): string => {
          try {
            const withoutQuery = url.split('?')[0];
            const parts = withoutQuery.split('/');
            return decodeURIComponent(parts[parts.length - 1] || url);
          } catch {
            return url;
          }
        };

        // 生成文件的指纹（用于跨平台去重）
        // 优先级：文件名 > 版本号+loader组合
        const getFileFingerprint = (tags: string[], url: string): string => {
          const filename = extractFilename(url);
          if (filename && filename.length > 3) {
            // 使用小写文件名作为指纹（忽略扩展名差异，如 .jar vs .zip 也应去重）
            const lower = filename.toLowerCase();
            // 去除扩展名
            const withoutExt = lower.replace(/\.(jar|zip|mrpack|rar|7z)$/i, '');
            return `fn:${withoutExt}`;
          }
          // fallback：用 tags 的前几项组合
          const sigParts = tags.slice(0, 3).join('|').toLowerCase();
          return `tg:${sigParts}`;
        };

        for (const result of [cfResult, mrResult]) {
          if (!result.ok) continue;
          try {
            const parsed = JSON.parse(result.data) as ModFiles;
            if (!parsed || Object.keys(parsed).length === 0) continue;
            if (firstNonEmpty === null) firstNonEmpty = result.source;
            for (const [mcVersion, files] of Object.entries(parsed)) {
              if (!merged[mcVersion]) {
                merged[mcVersion] = [];
              }
              // 已存在文件的指纹集合
              const existingFingerprints = new Set(
                merged[mcVersion].map(([t, u]) => getFileFingerprint(t, u))
              );
              // 已存在 URL 集合（兜底）
              const existingUrls = new Set(merged[mcVersion].map(([, url]) => url));

              for (const f of files) {
                const fp = getFileFingerprint(f[0], f[1]);
                if (existingFingerprints.has(fp)) continue;
                if (existingUrls.has(f[1])) continue;
                merged[mcVersion].push(f);
                existingFingerprints.add(fp);
                existingUrls.add(f[1]);
              }
            }
          } catch (err) {
            console.warn("Failed to parse " + result.source + " file data", err);
          }
        }

        if (Object.keys(merged).length > 0) {
          setModFiles(merged);
          setDataSource(firstNonEmpty || "CurseForge");
          const firstKey = Object.keys(merged).sort(compareMinecraftVersionDescending)[0];
          if (firstKey) {
            setExpandedVersions(new Set([firstKey]));
          }
        } else {
          throw new Error("All data sources returned no valid files");
        }
      } catch (error) {
        console.error("Failed to get mod files:", error);
        setFilesError(String(error));
      } finally {
        setLoadingFiles(false);
      }
    })();

    await Promise.all([infoPromise, filesPromise]);

    setLoading(false);
  };

  const toggleVersion = (mcVersion: string) => {
    setExpandedVersions(prev => {
      const next = new Set(prev);
      if (next.has(mcVersion)) {
        next.delete(mcVersion);
      } else {
        next.add(mcVersion);
      }
      return next;
    });
  };

  const handleDownload = async (file: ParsedFile, mcVersion: string) => {
    const status = getDownloadStatus(file.url);
    if (status === "downloading") return;
    if (status === "success") return;

    const modName = liveInfo?.title || liveInfo?.slug || modId;
    const modSlug = liveInfo?.slug || modId;

    // Infer mod loader from file information (unified parsing by tags, not file extension)
    // Priority: 1. UI subtitle loaderLabel (e.g., "Ornithe", "NeoForge")
    //           2. Fallback to hasXxx flag inference
    let modLoader = "universal";
    if (file.loaderLabel && file.loaderLabel !== "Universal") {
      modLoader = file.loaderLabel;
    } else {
      if (file.hasNeoForge) modLoader = "neoforge";
      else if (file.hasFabric) modLoader = "fabric";
      else if (file.hasQuilt) modLoader = "quilt";
      else if (file.hasLiteLoader) modLoader = "liteloader";
      else if (file.hasOrnithe) modLoader = "ornithe";
      else if (file.hasForge) modLoader = "forge";
    }

    // Determine resource kind based on project type (affects cache directory)
    // - mod / minecraft mod -> "mod"
    // - resourcepack / texture pack -> "resourcepack"
    // - shader -> "shaderpack"
    // - datapack -> "datapack"
    // - world / map -> "world"
    // - modpack / mod pack -> "modpack"
    // If forceType is set (e.g. from mcmod search), use it directly to avoid misclassification
    const projectType = (forceType || liveInfo?.projectType || "mod").toLowerCase();
    let resourceKind = "mod";
    if (projectType.includes("resourcepack") || projectType.includes("resource pack") || projectType.includes("texture pack")) {
      resourceKind = "resourcepack";
    } else if (projectType.includes("shader")) {
      resourceKind = "shaderpack";
    } else if (projectType.includes("datapack") || projectType.includes("data pack")) {
      resourceKind = "datapack";
    } else if (projectType.includes("modpack") || projectType.includes("mod pack")) {
      resourceKind = "modpack";
    } else if (projectType.includes("world") || projectType.includes("map")) {
      resourceKind = "world";
    }

    try {
      // First occupy downloading status (show loading before taskId returns)
      setDownloadingUrlToTaskId(prev => {
        const next = new Map(prev);
        if (!next.has(file.url)) {
          next.set(file.url, -1);  // -1 means waiting for backend to return taskId
        }
        return next;
      });

      const taskId = await startResourceDownload(
        resourceKind,
        modSlug,
        modName,
        mcVersion,
        modLoader,
        file.url
      );

      // Update to real taskId
      setDownloadingUrlToTaskId(prev => {
        const next = new Map(prev);
        next.set(file.url, taskId);
        return next;
      });
    } catch (err) {
      console.error("Download failed:", err);
      setDownloadingUrlToTaskId(prev => {
        const next = new Map(prev);
        next.delete(file.url);
        return next;
      });
      return;
    }

    // Dependency resolution runs after primary download is successfully started
    // Errors here should not affect the primary download's state
    if (settings.general.autoDownloadModDependencies && resourceKind === "mod") {
      let hostname: string | null = null;
      try {
        hostname = new URL(file.url).hostname;
      } catch (error) {
        console.warn("Failed to parse URL for dependency resolution:", error);
      }

      if (!hostname) return;

      const dependencyCommand = /(^|\.)modrinth\.com$/i.test(hostname)
        ? "get_modrinth_required_dependencies"
        : /(^|\.)forgecdn\.net$/i.test(hostname) || /(^|\.)curseforge\.com$/i.test(hostname)
          ? "get_curseforge_required_dependencies"
          : null;

      if (!dependencyCommand) return;

      void invoke<ResolvedModDependency[]>(dependencyCommand, {
        projectSlug: modSlug,
        mcVersion,
        modLoader,
        downloadUrl: file.url,
      })
        .then(async (dependencies) => {
          await Promise.all(
            dependencies.map(async (dependency) => {
              try {
                await startResourceDownload(
                  "mod",
                  dependency.projectSlug || dependency.projectId,
                  `${dependency.projectName} (dependency)`,
                  mcVersion,
                  modLoader,
                  dependency.downloadUrl,
                );
              } catch (error) {
                console.error(`Failed to download dependency ${dependency.projectName}:`, error);
              }
            }),
          );
        })
        .catch((error) => {
          // The selected mod download is already running; dependency lookup
          // failure should not turn it into a failed task.
          console.warn("Failed to resolve required mod dependencies:", error);
        });
    }
  };

  if (loading) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-muted-foreground">
        <Loader2 className="size-8 animate-spin" />
        <p className="text-sm">Loading project information...</p>
      </div>
    );
  }

  const displayTitle = liveInfo?.title || modId;
  const displaySlug = liveInfo?.slug || modId;
  const totalFiles = parsedFiles.size;
  const totalVersions = Array.from(parsedFiles.values()).reduce((sum, arr) => sum + arr.length, 0);

  return (
    <div className="flex h-full flex-col p-4">
      <Button variant="ghost" size="sm" className="w-fit mb-4" onClick={() => {
        const params = new URLSearchParams(window.location.search);
        const returnTarget = params.get("returnTo");
        if (returnTarget === "english") {
          const query = params.get("query") || "";
          const category = params.get("category") || "mod";
          router.push(`/download?tab=english&query=${encodeURIComponent(query)}&category=${encodeURIComponent(category)}`);
          return;
        }
        if (returnTarget === "chinese") {
          router.push("/download?tab=chinese");
          return;
        }
        router.push("/download");
      }}>
        <ArrowLeft className="mr-2 size-4" />
        Back to Search
      </Button>

      <div className="flex-1 min-h-0 flex flex-col gap-4 overflow-y-auto">
        <div className="rounded-xl border border-border bg-card p-5">
          <div className="flex gap-5 items-start flex-col md:flex-row">
            <div className={"shrink-0 w-20 h-20 " + (liveInfo?.iconUrl ? "" : "bg-gradient-to-br from-primary/20 to-primary/5") + " rounded-xl flex items-center justify-center border border-border overflow-hidden"}>
              {liveInfo?.iconUrl ? (
                <img
                  src={liveInfo.iconUrl}
                  alt=""
                  className="w-full h-full object-contain"
                  onError={(e) => {
                    const target = e.currentTarget;
                    if (target.parentElement) {
                      target.style.display = 'none';
                    }
                  }}
                />
              ) : (
                <Package className="size-8 text-primary" />
              )}
            </div>

            <div className="flex-1 min-w-0">
              <h1 className="text-2xl font-bold tracking-tight">{displayTitle}</h1>
              <div className="flex flex-wrap gap-2 mt-2 items-center">
                <Badge variant="secondary" className="text-xs font-mono">
                  {displaySlug}
                </Badge>

                {liveInfo?.source === 'both' && (
                  <Badge variant="outline" className="text-xs bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 border-emerald-500/30">
                    Modrinth + CurseForge
                  </Badge>
                )}
                {liveInfo?.source === 'modrinth' && (
                  <Badge variant="outline" className="text-xs bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 border-emerald-500/30">
                    Modrinth
                  </Badge>
                )}
                {liveInfo?.source === 'curseforge' && (
                  <Badge variant="outline" className="text-xs bg-orange-500/10 text-orange-700 dark:text-orange-300 border-orange-500/30">
                    CurseForge
                  </Badge>
                )}

                {dataSource && (
                  <Badge variant="outline" className="text-xs">
                    Files from {dataSource}
                  </Badge>
                )}

                <Badge variant="outline" className="text-xs">
                  {totalFiles} MC versions · {totalVersions} files
                </Badge>
              </div>

              <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground mt-3">
                {liveInfo?.downloads != null && (
                  <span>⬇ Total downloads {formatDownloads(liveInfo.downloads)}</span>
                )}
                {liveInfo?.author && (
                  <span>Author: <span className="text-foreground font-medium">{liveInfo.author}</span></span>
                )}
                {liveInfo?.updated && (
                  <span>Updated {formatDateShort(liveInfo.updated)}</span>
                )}
                {liveInfo?.projectType && (
                  <span>Type: {translateProjectType(liveInfo.projectType)}</span>
                )}
              </div>

              {liveInfo?.description && (
                <p className="text-sm text-muted-foreground mt-3 leading-relaxed">
                  {liveInfo.description}
                </p>
              )}

              <div className="flex flex-wrap items-center gap-2 mt-4">
                {liveInfo?.modrinthUrl && (
                  <Button variant="outline" size="sm" onClick={() => openExternalUrl(liveInfo.modrinthUrl!)}>
                    <ExternalLink className="mr-1.5 size-3.5" />
                    View on Modrinth
                  </Button>
                )}
                {liveInfo?.curseforgeUrl && (
                  <Button variant="outline" size="sm" onClick={() => openExternalUrl(liveInfo.curseforgeUrl!)}>
                    <ExternalLink className="mr-1.5 size-3.5" />
                    View on CurseForge
                  </Button>
                )}
                {liveInfo?.mcmodUrl && (
                  <Button variant="outline" size="sm" onClick={() => openExternalUrl(liveInfo.mcmodUrl!)}>
                    <ExternalLink className="mr-1.5 size-3.5" />
                    View on MCMod
                  </Button>
                )}
                {liveError && (
                  <span className="text-[11px] text-destructive">Failed to load online information: {liveError}</span>
                )}
              </div>
            </div>
          </div>
        </div>

        {modFiles && totalVersions > 0 ? (
          <div className="flex-1 min-h-0 space-y-2">
            {loadingFiles && (
              <div className="flex items-center justify-center gap-2 text-muted-foreground py-4">
                <Loader2 className="size-4 animate-spin" />
                <span className="text-sm">Loading file list...</span>
              </div>
            )}

            {Array.from(parsedFiles.entries()).map(([mcVersion, files]) => {
              const isExpanded = expandedVersions.has(mcVersion);
              const releaseCount = files.filter(f => f.isRelease).length;
              const nonReleaseCount = files.length - releaseCount;

              return (
                <div key={mcVersion} className="rounded-xl border border-border bg-card overflow-hidden">
                  <button onClick={() => toggleVersion(mcVersion)} className="w-full flex items-center justify-between p-4 hover:bg-accent/40 transition-colors text-left">
                    <div className="flex items-center gap-3">
                      <div className={"transition-transform duration-300 " + (isExpanded ? "rotate-180" : "")}>
                        <ChevronDown className="size-5 text-muted-foreground" />
                      </div>
                      <div className="flex flex-col">
                        <span className="font-semibold text-base">MC {mcVersion}</span>
                        <span className="text-xs text-muted-foreground mt-0.5">
                          {files.length} files
                          {releaseCount > 0 && (<span className="ml-2">· {releaseCount} release(s)</span>)}
                          {nonReleaseCount > 0 && (<span className="ml-2">· {nonReleaseCount} beta(s)</span>)}
                        </span>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {releaseCount > 0 && (
                        <span className="inline-flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
                          <Shield className="size-5" />
                        </span>
                      )}
                      {nonReleaseCount > 0 && (
                        <span className="inline-flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400">
                          <FlaskConical className="size-5" />
                        </span>
                      )}
                    </div>
                  </button>

                  <div className={"grid transition-all duration-300 ease-out " + (isExpanded ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0")}>
                    <div className="overflow-hidden">
                      <div className="border-t border-border divide-y divide-border/60 bg-muted/10">
                        {files.map((file, index) => {
                          const status = getDownloadStatus(file.url);
                          return (
                            <div key={index} className="flex items-center gap-3 px-4 py-3 hover:bg-accent/20 transition-colors">
                              <div className="flex items-center gap-2 shrink-0 w-16 justify-center">
                                {file.isRelease ? (
                                  <Shield className="size-7 text-emerald-500" aria-label="Release" />
                                ) : (
                                  <FlaskConical className="size-7 text-amber-500" aria-label="Beta" />
                                )}
                              </div>

                              <div className="flex-1 min-w-0">
                                <div className="text-sm font-semibold truncate">
                                  {file.versionLabel}
                                  {file.loaderLabel === "Universal" && (
                                    <span className="text-muted-foreground/70 ml-1" aria-label="Loader not recognized">:</span>
                                  )}
                                </div>
                                <div className="flex flex-wrap items-center gap-1.5 mt-1">
                                  <Badge variant={file.isRelease ? "secondary" : "outline"} className="text-[10px] h-4">{file.loaderLabel}</Badge>
                                  <Badge variant={file.isRelease ? "default" : "outline"} className={"text-[10px] h-4 " + (file.isRelease ? "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 border-emerald-500/30" : "text-amber-600 dark:text-amber-400 border-amber-500/30")}>{file.isRelease ? "Release" : "Beta"}</Badge>
                                  {file.serverLabel && (
                                    <Badge variant="outline" className="text-[10px] h-4 text-sky-600 dark:text-sky-400 border-sky-500/30">{file.serverLabel}</Badge>
                                  )}
                                  {file.cleanTags.slice(0, 2).map((tag, i) => (
                                    <span key={i} className="text-[10px] text-muted-foreground">{tag}</span>
                                  ))}
                                </div>
                              </div>

                              <Button size="sm" variant={status === "success" ? "secondary" : status === "error" ? "destructive" : "default"} disabled={status === "downloading" || status === "success"} onClick={() => handleDownload(file, mcVersion)} className="shrink-0">
                                {status === "downloading" && (<><Loader2 className="mr-1.5 size-3.5 animate-spin" /> Downloading</>)}
                                {status === "success" && (<><CheckCircle2 className="mr-1.5 size-3.5" /> Downloaded</>)}
                                {status === "error" && (<><XCircle className="mr-1.5 size-3.5" /> Retry</>)}
                                {status === "idle" && (<><Download className="mr-1.5 size-3.5" /> Download</>)}
                              </Button>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="flex-1 flex flex-col items-center justify-center gap-3 text-muted-foreground rounded-xl border border-dashed border-border bg-card/50 p-8">
            {filesError ? (
              <>
                <div className="size-10 rounded-full bg-destructive/10 flex items-center justify-center">
                  <span className="text-destructive text-lg">!</span>
                </div>
                <p className="text-sm font-medium text-foreground">Cannot connect to data source</p>
                <p className="text-xs text-muted-foreground text-center max-w-sm leading-relaxed">
                  Network error occurred while fetching file list. Connection interruptions or timeouts may occur when accessing overseas data sources.
                </p>
                <div className="mt-2 max-w-md w-full p-3 rounded-lg bg-muted/50 text-xs font-mono break-all text-muted-foreground">{filesError}</div>
                <div className="flex flex-wrap gap-2 mt-2">
                  <Button variant="default" size="sm" onClick={loadModData}>Retry</Button>
                  {liveInfo?.modrinthUrl && (
                    <Button variant="outline" size="sm" onClick={() => openExternalUrl(liveInfo.modrinthUrl!)}>
                      <ExternalLink className="mr-1.5 size-3.5" />
                      View on Modrinth
                    </Button>
                  )}
                  {liveInfo?.curseforgeUrl && (
                    <Button variant="outline" size="sm" onClick={() => openExternalUrl(liveInfo.curseforgeUrl!)}>
                      <ExternalLink className="mr-1.5 size-3.5" />
                      View on CurseForge
                    </Button>
                  )}
                </div>
              </>
            ) : (
              <>
                <Package className="size-10 opacity-40" />
                <p className="text-sm">No mod files available</p>
                <p className="text-xs text-muted-foreground">Please check if the mod slug is correct, or try again later</p>
                <Button variant="outline" size="sm" className="mt-2" onClick={loadModData}>Reload</Button>
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}