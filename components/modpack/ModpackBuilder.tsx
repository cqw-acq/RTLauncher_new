"use client";

import { useState, useEffect, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft,
  Search,
  Box,
  Package,
  Plus,
  Trash2,
  Save,
  Download,
  Folder,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Server,
  Monitor,
  ChevronDown,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useRouter } from "next/navigation";
import {
  ModrinthFileEntry,
  CurseforgeFileEntry,
  getModpackDir,
  saveInstance,
  exportInstance,
  formatTimestamp,
} from "@/components/modpack/modpack-api";
import { useMinecraftVersions } from "@/hooks/use-minecraft-versions";
import { useI18n } from "@/components/i18n/use-i18n";
import type { AppLanguage } from "@/components/settings/settings-provider";
import {
  CURSEFORGE_MOD_LOADER_TYPES,
  CurseforgeApiFile,
  buildCurseforgeFilesUrl,
  filterCurseforgeFilesByGameVersion,
} from "@/components/modpack/curseforge-api";

// =============================================================================
// 搜索结果数据结构
// =============================================================================

type CategoryId = "mod" | "modpack" | "resourcepack" | "shaders" | "datapack" | "worlds";

interface SearchHit {
  slug: string;
  title: string;
  description?: string;
  iconUrl?: string;
  downloads?: number;
  categories?: string[];
  game_versions?: string[];
  updated?: string;
  author?: string;
  source: "modrinth" | "curseforge";
  project_type?: string;
  external_url?: string;
  client_side?: "required" | "optional" | "unsupported";
  server_side?: "required" | "optional" | "unsupported";
}

// =============================================================================
// CurseForge API Key
// =============================================================================

const CURSEFORGE_API_KEY = "$2a$10$VTAFCxje5a1Jkqv0aGWjQ.fULedAEPctDqppOkNMRVv.edVnG7KQ6";
const CURSEFORGE_HEADERS = {
  "x-api-key": CURSEFORGE_API_KEY,
  Accept: "application/json",
  "User-Agent": "RTLauncher/1.0",
};
const MODRINTH_HEADERS = {
  Accept: "application/json",
  "User-Agent": "RTLauncher/1.0",
};

interface ParsedModrinthVersion {
  id: string;
  project_id: string;
  version_number: string;
  game_versions: string[];
  loaders: string[];
  files: Array<{
    url: string;
    filename: string;
    primary: boolean;
    size: number;
    hashes: { sha1: string; sha512: string; sha256?: string };
  }>;
  date_published: string;
  version_type: string;
  dependencies: Array<{
    version_id?: string | null;
    project_id?: string | null;
    dependency_type: string;
  }>;
}

function pickPrimaryFile(v: any): ParsedModrinthVersion["files"][number] | null {
  if (!v || !Array.isArray(v.files) || v.files.length === 0) return null;
  const primary = v.files.find((f: any) => f.primary === true);
  return primary || v.files[0];
}

function defaultSubfolderForCategory(cat: CategoryId, projectType?: string): string {
  const t = (projectType || cat).toLowerCase();
  if (t.startsWith("modpack")) return "";
  if (t.startsWith("resourcepack") || t === "resource pack") return "resourcepacks";
  if (t.startsWith("shader")) return "shaderpacks";
  if (t.startsWith("datapack") || t === "data pack") return "datapacks";
  if (t.startsWith("world")) return "saves";
  return "mods";
}

function parseModrinthVersion(v: any): ParsedModrinthVersion | null {
  const primary = pickPrimaryFile(v);
  if (!primary || !v?.id) return null;
  return {
    id: String(v.id),
    project_id: String(v.project_id || ""),
    version_number: String(v.version_number || ""),
    game_versions: Array.isArray(v.game_versions) ? v.game_versions : [],
    loaders: Array.isArray(v.loaders) ? v.loaders : [],
    files: [primary],
    date_published: String(v.date_published || ""),
    version_type: String(v.version_type || "release"),
    dependencies: Array.isArray(v.dependencies) ? v.dependencies : [],
  };
}

function environmentRequirementLabel(
  value: "required" | "optional" | "unsupported",
): string {
  if (value === "required") return "必须";
  if (value === "optional") return "可选";
  return "不支持";
}

// =============================================================================
// MC 版本：模糊匹配
// =============================================================================

function findBestMatchingVersion(
  query: string,
  versions: { id: string; type: string; releaseDate: string }[],
): { id: string; type: string; releaseDate: string } | null {
  const q = query.trim().toLowerCase();
  if (!q) return null;
  // 1) 精确匹配
  const exact = versions.find((v) => v.id.toLowerCase() === q);
  if (exact) return exact;
  // 2) 前缀匹配（优先 release）
  const prefixMatches = versions.filter((v) => v.id.toLowerCase().startsWith(q));
  if (prefixMatches.length > 0) {
    const release = prefixMatches.find((v) => v.type === "release");
    return release || prefixMatches[0];
  }
  // 3) 包含匹配
  const contains = versions.filter((v) => v.id.toLowerCase().includes(q));
  if (contains.length > 0) {
    const release = contains.find((v) => v.type === "release");
    return release || contains[0];
  }
  return null;
}

// =============================================================================
// 主组件
// =============================================================================

export function ModpackBuilder({
  format,
  initialName,
  gameVersion,
  existingFiles,
  initialLoader,
  initialLoaderVersion,
  initialPackVersion,
  initialAuthor,
  initialOptifine,
  initialOptifineVersion,
  initialCrossLoader,
}: {
  format: "modrinth" | "curseforge";
  initialName?: string;
  gameVersion?: string;
  existingFiles?: (ModrinthFileEntry | CurseforgeFileEntry)[];
  initialLoader?: string;
  initialLoaderVersion?: string;
  initialPackVersion?: string;
  initialAuthor?: string;
  initialOptifine?: boolean;
  initialOptifineVersion?: string;
  initialCrossLoader?: boolean;
}) {
  const router = useRouter();
  const { t, language } = useI18n();
  const L = <T extends { [k in AppLanguage]: string }>(obj: T): string => obj[language] ?? obj["zh-CN"] ?? obj["en-US"];
  // 顶部元数据
  const [name, setName] = useState(initialName || "");
  const [packVersion, setPackVersion] = useState(initialPackVersion || "1.0.0");
  const [author, setAuthor] = useState(initialAuthor || "");
  const [gameVer, setGameVer] = useState(gameVersion || "");
  const [category, setCategory] = useState<CategoryId>("mod");
  const [query, setQuery] = useState("");
  const [dir, setDir] = useState("");

  // 加载器
  const ALL_LOADERS = ["forge", "neoforge", "fabric", "quilt", "liteloader"] as const;
  const [selectedLoader, setSelectedLoader] = useState<string>(
    initialLoader && ALL_LOADERS.includes(initialLoader as any)
      ? initialLoader
      : "forge",
  );
  const [loaderVersion, setLoaderVersion] = useState(initialLoaderVersion || "");
  const [loaderVersionOptions, setLoaderVersionOptions] = useState<string[]>([]);
  const [loaderVersionsLoading, setLoaderVersionsLoading] = useState(false);
  const [loaderVersionsError, setLoaderVersionsError] = useState<string | null>(null);

  // OptiFine
  const [useOptifine, setUseOptifine] = useState<boolean>(initialOptifine || false);
  // 信雅互联模式：开启后 mod 搜索同时覆盖 forge + fabric，解除 loader 限制
  const [crossLoader, setCrossLoader] = useState<boolean>(initialCrossLoader || false);
  const [optifineVersions, setOptifineVersions] = useState<
    { id: string; type_: string; mcversion: string; patch: string; filename: string; forge: string }[]
  >([]);
  const [optifineLoading, setOptifineLoading] = useState(false);
  const [selectedOptifineVersion, setSelectedOptifineVersion] = useState<string>(
    initialOptifineVersion || "",
  );

  // MC 版本列表 + 下拉
  const { versions: mcVersions, loading: mcLoading, error: mcError } =
    useMinecraftVersions();
  const [versionDropdownOpen, setVersionDropdownOpen] = useState(false);

  // 搜索状态
  const [results, setResults] = useState<SearchHit[] | null>(null);
  const [searchLoading, setSearchLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  // 当前正在查看的项目
  const [activeHit, setActiveHit] = useState<SearchHit | null>(null);
  const [activeLoading, setActiveLoading] = useState(false);
  const [activeError, setActiveError] = useState<string | null>(null);
  const [addError, setAddError] = useState<string | null>(null);
  const [addingFileKey, setAddingFileKey] = useState<string | null>(null);
  const [modrinthVersions, setModrinthVersions] = useState<ParsedModrinthVersion[]>([]);
  const [curseforgeFiles, setCurseforgeFiles] = useState<CurseforgeApiFile[]>([]);
  const [curseforgeProjectId, setCurseforgeProjectId] = useState<number | null>(null);
  const [curseforgeDisplayName, setCurseforgeDisplayName] = useState<string>("");
  const detailsRef = useRef<HTMLDivElement>(null);

  // 已选文件（兼容旧格式：client/server 顶层字段 → env 嵌套字段）
  const [selectedModrinth, setSelectedModrinth] = useState<ModrinthFileEntry[]>(
    ((existingFiles as ModrinthFileEntry[] | undefined)?.filter((f) => (f as any).hashes) || []).map(
      (f) => {
        const old = f as any;
        return {
          path: f.path,
          hashes: f.hashes,
          env: old.env
            ? f.env
            : {
                client: (old.client as "required" | "optional" | "unsupported") || "required",
                server: (old.server as "required" | "optional" | "unsupported") || "required",
              },
          downloads: f.downloads,
          fileSize: f.fileSize,
          display_name: f.display_name,
        };
      },
    ),
  );
  const [selectedCurseforge, setSelectedCurseforge] = useState<CurseforgeFileEntry[]>(
    (existingFiles as CurseforgeFileEntry[] | undefined)?.filter(
      (f) => (f as any).projectID !== undefined,
    ) || [],
  );

  // 保存状态
  const [saveStatus, setSaveStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const [saveMessage, setSaveMessage] = useState("");

  // 选中的 MC 版本（显示为"已选 X"）
  const matchedVersion = useMemo(() => {
    if (!gameVer.trim() || mcVersions.length === 0) return null;
    return findBestMatchingVersion(gameVer, mcVersions);
  }, [gameVer, mcVersions]);

  const mcVersionValid = matchedVersion !== null;

  // 使用下载器现有的后端接口读取真实加载器版本，制作器只允许从列表选择。
  useEffect(() => {
    if (!matchedVersion?.id) {
      setLoaderVersionOptions([]);
      setLoaderVersionsError(null);
      return;
    }

    let cancelled = false;
    const command =
      selectedLoader === "forge"
        ? "get_forge_versions"
        : selectedLoader === "neoforge"
          ? "get_neoforge_versions"
          : selectedLoader === "fabric"
            ? "get_fabric_loader_versions"
            : selectedLoader === "quilt"
              ? "get_quilt_loader_versions"
              : "get_liteloader_versions";

    (async () => {
      setLoaderVersionsLoading(true);
      setLoaderVersionsError(null);
      setLoaderVersionOptions([]);
      try {
        const result = await invoke<{ id: string; version: string }[]>(command, {
          mcVersion: matchedVersion.id,
          ...(selectedLoader === "fabric" ? { useMirror: true } : {}),
        });
        if (cancelled) return;
        const prefix = `${matchedVersion.id}-`;
        const versions = Array.from(
          new Set(
            (result || [])
              .map((item) => String(item.version || item.id || "").trim())
              .map((version) =>
                (selectedLoader === "forge" || selectedLoader === "neoforge") &&
                version.startsWith(prefix)
                  ? version.slice(prefix.length)
                  : version,
              )
              .filter(Boolean),
          ),
        );
        setLoaderVersionOptions(versions);
        setLoaderVersion((current) => (versions.includes(current) ? current : ""));
        if (versions.length === 0) {
          setLoaderVersionsError("当前 Minecraft 版本没有对应的加载器版本");
        }
      } catch (error: any) {
        if (!cancelled) {
          setLoaderVersionOptions([]);
          setLoaderVersionsError(error?.message || String(error));
        }
      } finally {
        if (!cancelled) setLoaderVersionsLoading(false);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [matchedVersion?.id, selectedLoader]);

  useEffect(() => {
    getModpackDir().then(setDir);
  }, []);

  // MC 版本变化时，如果勾选了 OptiFine，自动获取对应版本的 OptiFine 列表
  useEffect(() => {
    if (!useOptifine || !matchedVersion) {
      setOptifineVersions([]);
      return;
    }
    let cancelled = false;
    (async () => {
      setOptifineLoading(true);
      try {
        const data = await invoke<
          { id: string; type_: string; mcversion: string; patch: string; filename: string; forge: string }[]
        >("get_optifine_versions", { mcVersion: matchedVersion.id });
        if (!cancelled) {
          setOptifineVersions(data || []);
          if (selectedOptifineVersion && !(data || []).some((v) => v.filename === selectedOptifineVersion)) {
            setSelectedOptifineVersion("");
          }
        }
      } catch (err) {
        console.error("获取 OptiFine 版本失败:", err);
        if (!cancelled) setOptifineVersions([]);
      } finally {
        if (!cancelled) setOptifineLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [useOptifine, matchedVersion?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // ===========================================================================
  // 搜索（带 MC 版本 + 加载器过滤）
  // ===========================================================================
  const doSearch = async () => {
    const q = query.trim();
    if (!q) {
      setSearchError(t("modpack.modpackBuilder.enterASearchTerm"));
      return;
    }
    if (!mcVersionValid) {
      setSearchError(t("modpack.modpackBuilder.enterAValidMinecraftVersionFirst"));
      return;
    }
    const targetMcVersion = matchedVersion!.id;

    setSearchLoading(true);
    setSearchError(null);
    setResults(null);
    setActiveHit(null);
    setModrinthVersions([]);
    setCurseforgeFiles([]);

    try {
      const hits: SearchHit[] = [];

      // Modrinth
      if (format === "modrinth") {
        const modrinthProjectType =
          category === "shaders" ? "shader" : category === "worlds" ? "world" : category;
        // 仅对 mod 类别做加载器过滤；其余类别（shader/resourcepack/datapack/worlds/modpack）一律不过滤 loader
        // 若开启信雅互联，则 mod 类别也解除 loader 限制（同时搜索 forge+fabric+neoforge+quilt+liteloader）
        const needLoaderFilter = category === "mod" && !crossLoader;
        // 搜索整合包时先展示完整的关键词结果，再在版本列表中标出与当前
        // 工程 MC 版本的兼容性；否则 Better MC 一类项目会在搜索阶段被排除，
        // Modrinth 的模糊搜索反而可能只留下名称无关的结果。
        const facetGroups: string[][] = [[`project_type:${modrinthProjectType}`]];
        if (category !== "modpack") {
          facetGroups.push([`versions:${targetMcVersion}`]);
        }
        if (needLoaderFilter) {
          facetGroups.push([`categories:${selectedLoader}`]);
        }
        const facets = encodeURIComponent(JSON.stringify(facetGroups));
        const url = `https://api.modrinth.com/v2/search?query=${encodeURIComponent(q)}&limit=30&facets=${facets}`;
        const res = await fetch(url, { headers: MODRINTH_HEADERS, cache: "no-store" });
        if (res.ok) {
          const data = await res.json();
          for (const hit of data?.hits || []) {
            hits.push({
              slug: hit.slug,
              title: hit.title,
              description: hit.description,
              iconUrl: hit.icon_url,
              downloads: hit.downloads,
              categories: hit.categories,
              game_versions: hit.game_versions,
              updated: hit.date_modified || hit.date_updated,
              author: hit.author,
              source: "modrinth",
              project_type: hit.project_type,
              client_side: ["required", "optional", "unsupported"].includes(hit.client_side)
                ? hit.client_side
                : undefined,
              server_side: ["required", "optional", "unsupported"].includes(hit.server_side)
                ? hit.server_side
                : undefined,
              external_url: `https://modrinth.com/${hit.project_type || "mod"}/${hit.slug}`,
            });
          }
        }
      }

      // CurseForge
      if (format === "curseforge") {
        const classIdMap: Record<CategoryId, number> = {
          mod: 6,
          modpack: 4471,
          resourcepack: 12,
          shaders: 6552,
          datapack: 6949,
          worlds: 17,
        };
        // CurseForge: modLoaderType 只对 mod 类别有用
        // 仅对 mod 类别做加载器过滤；信雅互联模式下解除限制
        const needLoaderFilter = category === "mod" && !crossLoader;
        const modLoaderType =
          needLoaderFilter ? CURSEFORGE_MOD_LOADER_TYPES[selectedLoader] : undefined;
        const url =
          `https://api.curseforge.com/v1/mods/search?gameId=432&searchFilter=${encodeURIComponent(q)}&pageSize=30&sortField=5&sortOrder=desc&classId=${classIdMap[category]}` +
          (category === "modpack"
            ? ""
            : `&gameVersion=${encodeURIComponent(targetMcVersion)}`) +
          (modLoaderType !== undefined ? `&modLoaderType=${modLoaderType}` : "");
        const res = await fetch(url, { headers: CURSEFORGE_HEADERS, cache: "no-store" });
        if (res.ok) {
          const data = await res.json();
          for (const item of data?.data || []) {
            hits.push({
              slug: item.slug || String(item.id),
              title: item.name,
              description: item.summary,
              iconUrl: item.logo?.thumbnailUrl || item.logo?.url,
              downloads: item.downloadCount,
              categories: (item.categories || []).map((c: any) => c.name),
              game_versions: (item.latestFilesIndexes || [])
                .map((f: any) => f.gameVersion)
                .filter(Boolean),
              updated: item.dateModified || item.dateReleased,
              author: (item.authors || []).map((a: any) => a.name).join(", "),
              source: "curseforge",
              project_type: category,
              external_url:
                item.links?.websiteUrl ||
                `https://www.curseforge.com/minecraft/mc-mods/${item.slug || item.id}`,
            });
          }
        }
      }

      // 整合包搜索保留平台返回的关键词相关度顺序；其他资源继续按下载量排列。
      if (category !== "modpack") {
        hits.sort((a, b) => (b.downloads || 0) - (a.downloads || 0));
      }
      setResults(hits);
      if (hits.length === 0) {
        const filterDesc =
          category === "mod"
            ? crossLoader
              ? `${targetMcVersion} (${t("modpack.modpackBuilder.loaderRestrictionRemoved")})`
              : `${targetMcVersion} + ${selectedLoader}`
            : category === "modpack"
              ? L({ "zh-CN": `关键词“${q}”`, "en-US": `keyword “${q}”` })
        : targetMcVersion;
        setSearchError(L({ "zh-CN": `未找到匹配 ${filterDesc} 的项目`, "en-US": `No projects match ${filterDesc}` }));
      }
    } catch (e: any) {
      setSearchError(`${t("modpack.modpackBuilder.searchFailed")}: ${e?.message || e}`);
    } finally {
      setSearchLoading(false);
    }
  };

  // ===========================================================================
  // 展开项目 + 获取版本/文件（过滤 MC 版本 + 加载器）
  // ===========================================================================
  const openHit = async (hit: SearchHit) => {
    setActiveHit(hit);
    setActiveLoading(true);
    setActiveError(null);
    setAddError(null);
    setModrinthVersions([]);
    setCurseforgeFiles([]);

    const targetMcVersion = matchedVersion?.id;

    try {
      if (format === "modrinth") {
        const data = await fetch(
          `https://api.modrinth.com/v2/project/${encodeURIComponent(hit.slug)}/version`,
          { headers: MODRINTH_HEADERS, cache: "no-store" },
        );
        if (!data.ok) {
          throw new Error(`Modrinth API ${t("modpack.modpackBuilder.requestFailed")} (${data.status})`);
        }
        const json = await data.json();
        const list: ParsedModrinthVersion[] = [];
        if (Array.isArray(json)) {
          for (const v of json) {
            if (targetMcVersion && category !== "modpack") {
              const gv: string[] = Array.isArray(v.game_versions) ? v.game_versions : [];
              if (!gv.includes(targetMcVersion)) continue;
            }
            // 仅对 mod 类别做加载器过滤；其余类别不过滤 loader
            // 若开启信雅互联，则 mod 类别也不过滤（同时展示 forge fabric neoforge quilt liteloader）
            const needLoaderFilter = category === "mod" && !crossLoader;
            if (needLoaderFilter) {
              const loaders: string[] = Array.isArray(v.loaders) ? v.loaders : [];
              if (!loaders.includes(selectedLoader)) continue;
            }
            const parsed = parseModrinthVersion(v);
            if (parsed) list.push(parsed);
          }
        }
        setModrinthVersions(list);
      } else {
        // CurseForge
        let proj = await fetchCurseforgeProjectId(hit.slug);
        if (!proj) {
          const asNum = parseInt(hit.slug, 10);
          if (!isNaN(asNum)) {
            proj = { projectId: asNum, projectName: hit.title };
          }
        }
        if (!proj) {
          throw new Error(t("modpack.modpackBuilder.theProjectCouldNotBeFoundOnCurseForge"));
        }
        setCurseforgeProjectId(proj.projectId);
        setCurseforgeDisplayName(proj.projectName);
        // 让 CurseForge 在服务端按 MC 版本/加载器筛选文件，避免依赖 File
        // 响应中不存在的 modLoaderType 字段。信雅互联模式仅取消加载器限制。
        const modLoaderType =
          category === "mod" && !crossLoader
            ? CURSEFORGE_MOD_LOADER_TYPES[selectedLoader]
            : undefined;
        const files = await fetchCurseforgeFiles(
          proj.projectId,
          category === "modpack" ? undefined : targetMcVersion,
          modLoaderType,
        );
        setCurseforgeFiles(files);
      }
    } catch (e: any) {
      setActiveError(e?.message || String(e));
    } finally {
      setActiveLoading(false);
      // 版本选择区位于搜索结果列表下方。项目加载完成后自动滚动过去，
      // 避免点击结果后看起来毫无反应。
      window.setTimeout(() => {
        detailsRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      }, 0);
    }
  };

  async function fetchCurseforgeProjectId(
    slug: string,
  ): Promise<{ projectId: number; projectName: string } | null> {
    const url = `https://api.curseforge.com/v1/mods/search?gameId=432&slug=${encodeURIComponent(slug)}&pageSize=1`;
    const res = await fetch(url, { headers: CURSEFORGE_HEADERS, cache: "no-store" });
    if (!res.ok) return null;
    const data = await res.json();
    const first = data?.data?.[0];
    if (!first) return null;
    return { projectId: first.id, projectName: first.name || slug };
  }

  async function fetchCurseforgeFiles(
    projectId: number,
    gameVersion?: string,
    modLoaderType?: number,
    readAllPages = true,
  ): Promise<CurseforgeApiFile[]> {
    const files: CurseforgeApiFile[] = [];
    let index = 0;
    const visitedIndexes = new Set<number>();

    // CurseForge 单页最多返回 50 个文件，持续翻页直到读取完整版本列表。
    while (!visitedIndexes.has(index)) {
      visitedIndexes.add(index);
      const url = buildCurseforgeFilesUrl(
        projectId,
        gameVersion,
        modLoaderType,
        index,
      );
      const res = await fetch(url, {
        headers: CURSEFORGE_HEADERS,
        cache: "no-store",
      });
      if (!res.ok) break;
      const data = await res.json();
      const page: CurseforgeApiFile[] = Array.isArray(data?.data) ? data.data : [];
      files.push(...page);
      if (!readAllPages) break;

      const pagination = data?.pagination;
      const resultCount = Number(pagination?.resultCount ?? page.length);
      const totalCount = Number(pagination?.totalCount ?? files.length);
      if (resultCount <= 0 || files.length >= totalCount) break;
      index = Number(pagination?.index ?? index) + resultCount;
    }

    // API 返回字段是 gameVersions（camelCase）。再做一层本地校验，
    // 防止服务端返回其他 MC 版本的文件；筛选为空时不再回退到全部文件。
    return filterCurseforgeFilesByGameVersion(files, gameVersion);
  }

  // ===========================================================================
  // 加入已选
  // ===========================================================================
  const createModrinthEntry = (
    hit: SearchHit,
    v: ParsedModrinthVersion,
    isDependency: boolean,
  ): ModrinthFileEntry => {
    const primary = v.files[0];
    if (!primary) throw new Error(`版本 ${v.version_number || v.id} 缺少下载文件`);
    const entryCategory = isDependency ? "mod" : category;
    const subfolder = defaultSubfolderForCategory(entryCategory, hit.project_type);
    const path = subfolder ? `${subfolder}/${primary.filename}` : primary.filename;
    return {
      path,
      hashes: {
        sha1: primary.hashes.sha1,
        sha512: primary.hashes.sha512 || "",
        sha256: primary.hashes.sha256,
      },
      env: {
        client: hit.client_side || "required",
        server: hit.server_side || "required",
      },
      downloads: [primary.url],
      fileSize: primary.size || 0,
      display_name: `${hit.title} — ${v.version_number}${isDependency ? "（依赖）" : ""}`,
    };
  };

  const fetchModrinthVersion = async (versionId: string) => {
    const response = await fetch(
      `https://api.modrinth.com/v2/version/${encodeURIComponent(versionId)}`,
      { headers: MODRINTH_HEADERS, cache: "no-store" },
    );
    if (!response.ok) {
      throw new Error(`读取 Modrinth 依赖版本失败 (${response.status})`);
    }
    const parsed = parseModrinthVersion(await response.json());
    if (!parsed) throw new Error(`Modrinth 依赖版本 ${versionId} 没有可下载文件`);
    return parsed;
  };

  const fetchModrinthProjectHit = async (
    projectId: string,
    fallbackTitle: string,
  ): Promise<SearchHit> => {
    const response = await fetch(
      `https://api.modrinth.com/v2/project/${encodeURIComponent(projectId)}`,
      { headers: MODRINTH_HEADERS, cache: "no-store" },
    );
    if (!response.ok) {
      throw new Error(`读取 Modrinth 依赖项目失败 (${response.status})`);
    }
    const project = await response.json();
    return {
      slug: String(project.slug || project.id || projectId),
      title: String(project.title || fallbackTitle || projectId),
      source: "modrinth",
      project_type: String(project.project_type || "mod"),
      client_side: ["required", "optional", "unsupported"].includes(project.client_side)
        ? project.client_side
        : "required",
      server_side: ["required", "optional", "unsupported"].includes(project.server_side)
        ? project.server_side
        : "required",
    };
  };

  const fetchCompatibleModrinthVersion = async (
    projectId: string,
    parentVersion: ParsedModrinthVersion,
  ) => {
    const params = new URLSearchParams();
    const targetMcVersion = matchedVersion?.id;
    if (targetMcVersion) params.set("game_versions", JSON.stringify([targetMcVersion]));
    const compatibleLoaders = parentVersion.loaders.filter(Boolean);
    if (compatibleLoaders.length > 0) {
      params.set("loaders", JSON.stringify(compatibleLoaders));
    }
    const queryString = params.toString();
    const response = await fetch(
      `https://api.modrinth.com/v2/project/${encodeURIComponent(projectId)}/version${queryString ? `?${queryString}` : ""}`,
      { headers: MODRINTH_HEADERS, cache: "no-store" },
    );
    if (!response.ok) {
      throw new Error(`读取 Modrinth 依赖文件失败 (${response.status})`);
    }
    const versions = await response.json();
    const parsed = Array.isArray(versions)
      ? versions.map(parseModrinthVersion).find(Boolean)
      : null;
    if (!parsed) {
      throw new Error(`依赖项目 ${projectId} 没有适配当前 Minecraft/加载器的版本`);
    }
    return parsed as ParsedModrinthVersion;
  };

  const collectModrinthFiles = async (
    hit: SearchHit,
    version: ParsedModrinthVersion,
    isDependency: boolean,
    visited: Set<string>,
  ): Promise<ModrinthFileEntry[]> => {
    if (visited.has(version.id)) return [];
    if (visited.size >= 200) throw new Error("Modrinth 依赖数量超过 200 个");
    visited.add(version.id);

    const entries = [createModrinthEntry(hit, version, isDependency)];
    if (!isDependency && category === "modpack") return entries;

    for (const dependency of version.dependencies) {
      if (dependency.dependency_type !== "required") continue;
      const dependencyVersion = dependency.version_id
        ? await fetchModrinthVersion(dependency.version_id)
        : dependency.project_id
          ? await fetchCompatibleModrinthVersion(dependency.project_id, version)
          : null;
      if (!dependencyVersion) continue;
      const projectId = dependency.project_id || dependencyVersion.project_id;
      if (!projectId) throw new Error("Modrinth 必需依赖缺少项目 ID");
      const dependencyHit = await fetchModrinthProjectHit(
        projectId,
        dependencyVersion.version_number,
      );
      entries.push(
        ...(await collectModrinthFiles(
          dependencyHit,
          dependencyVersion,
          true,
          visited,
        )),
      );
    }
    return entries;
  };

  const addModrinthFile = async (hit: SearchHit, version: ParsedModrinthVersion) => {
    const addingKey = `modrinth:${version.id}`;
    setAddingFileKey(addingKey);
    setAddError(null);
    try {
      const collected = await collectModrinthFiles(hit, version, false, new Set());
      const byPath = new Map<string, ModrinthFileEntry>();
      for (const entry of [...selectedModrinth, ...collected]) {
        const key = entry.path.toLowerCase();
        const existing = byPath.get(key);
        if (existing && existing.hashes.sha1 !== entry.hashes.sha1) {
          throw new Error(`目标路径存在不同版本：${entry.path}，请先移除旧版本`);
        }
        byPath.set(key, existing || entry);
      }
      setSelectedModrinth(Array.from(byPath.values()));
    } catch (error: any) {
      setAddError(error?.message || String(error));
    } finally {
      setAddingFileKey(null);
    }
  };

  const fetchCurseforgeProject = async (projectId: number) => {
    const response = await fetch(`https://api.curseforge.com/v1/mods/${projectId}`, {
      headers: CURSEFORGE_HEADERS,
      cache: "no-store",
    });
    if (!response.ok) {
      throw new Error(`读取 CurseForge 依赖项目失败 (${response.status})`);
    }
    const project = (await response.json())?.data;
    if (!project) throw new Error(`CurseForge 依赖项目 ${projectId} 缺少数据`);
    return project;
  };

  const inferCurseforgeLoaderType = (file: CurseforgeApiFile) => {
    const versions = (file.gameVersions || []).map((value) => value.toLowerCase());
    const matchedLoader = Object.keys(CURSEFORGE_MOD_LOADER_TYPES).find((loader) =>
      versions.some((value) => value === loader || value.includes(loader)),
    );
    return CURSEFORGE_MOD_LOADER_TYPES[matchedLoader || selectedLoader];
  };

  const collectCurseforgeFiles = async (
    projectId: number,
    projectName: string,
    file: CurseforgeApiFile,
    isDependency: boolean,
    visited: Set<number>,
  ): Promise<CurseforgeFileEntry[]> => {
    if (visited.has(projectId)) return [];
    if (visited.size >= 200) throw new Error("CurseForge 依赖数量超过 200 个");
    visited.add(projectId);

    const entries: CurseforgeFileEntry[] = [
      {
        projectID: projectId,
        fileID: file.id,
        display_name: `${projectName} — ${file.displayName || file.fileName || file.id}${isDependency ? "（依赖）" : ""}`,
        required: true,
        category: isDependency ? "mod" : category,
      },
    ];
    if (!isDependency && category === "modpack") return entries;

    for (const dependency of file.dependencies || []) {
      if (dependency.relationType !== 3 || !dependency.modId) continue;
      if (visited.has(dependency.modId)) continue;
      const dependencyProject = await fetchCurseforgeProject(dependency.modId);
      const compatibleFiles = await fetchCurseforgeFiles(
        dependency.modId,
        matchedVersion?.id,
        inferCurseforgeLoaderType(file),
        false,
      );
      const dependencyFile = compatibleFiles[0];
      if (!dependencyFile) {
        throw new Error(`依赖 ${dependencyProject.name || dependency.modId} 没有适配当前 Minecraft/加载器的文件`);
      }
      entries.push(
        ...(await collectCurseforgeFiles(
          dependency.modId,
          String(dependencyProject.name || dependency.modId),
          dependencyFile,
          true,
          visited,
        )),
      );
    }
    return entries;
  };

  const addCurseforgeFile = async (file: CurseforgeApiFile) => {
    if (!curseforgeProjectId) return;
    const addingKey = `curseforge:${file.id}`;
    setAddingFileKey(addingKey);
    setAddError(null);
    try {
      const collected = await collectCurseforgeFiles(
        curseforgeProjectId,
        curseforgeDisplayName || activeHit?.title || String(curseforgeProjectId),
        file,
        false,
        new Set(),
      );
      setSelectedCurseforge((previous) => {
        const byProject = new Map(previous.map((entry) => [entry.projectID, entry]));
        collected.forEach((entry, index) => {
          if (index === 0 || !byProject.has(entry.projectID)) {
            byProject.set(entry.projectID, entry);
          }
        });
        return Array.from(byProject.values());
      });
    } catch (error: any) {
      setAddError(error?.message || String(error));
    } finally {
      setAddingFileKey(null);
    }
  };

  const toggleCurseforgeRequired = (idx: number) => {
    setSelectedCurseforge((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], required: !next[idx].required };
      return next;
    });
  };

  // 移除
  const removeModrinth = (idx: number) => {
    setSelectedModrinth((prev) => prev.filter((_, i) => i !== idx));
  };
  const removeCurseforge = (idx: number) => {
    setSelectedCurseforge((prev) => prev.filter((_, i) => i !== idx));
  };

  // ===========================================================================
  // 保存
  // ===========================================================================
  const handleSave = async (silent = false) => {
    const trimmed = name.trim();
    if (!trimmed) {
      if (!silent) {
        setSaveStatus("error");
        setSaveMessage(L({ "zh-CN": "请先填写整合包名称", "en-US": "Enter a modpack name first" }));
      }
      return false;
    }
    if (!packVersion.trim()) {
      if (!silent) {
        setSaveStatus("error");
        setSaveMessage("请填写整合包版本，例如 1.0.0");
      }
      return false;
    }
    if (!loaderVersion.trim() || loaderVersion.trim().toLowerCase() === "latest") {
      if (!silent) {
        setSaveStatus("error");
        setSaveMessage("请填写加载器的具体版本，不能使用 latest");
      }
      return false;
    }
    if (format === "curseforge" && !author.trim()) {
      if (!silent) {
        setSaveStatus("error");
        setSaveMessage("CurseForge 整合包需要填写作者");
      }
      return false;
    }
    if (format === "modrinth" && selectedLoader === "liteloader") {
      if (!silent) {
        setSaveStatus("error");
        setSaveMessage("Modrinth mrpack 规范不支持 LiteLoader 依赖");
      }
      return false;
    }
    if (!mcVersionValid) {
      if (!silent) {
        setSaveStatus("error");
        setSaveMessage(t("modpack.modpackBuilder.theMinecraftVersionIsInvalidAndCannotBeSaved"));
      }
      return false;
    }

    setSaveStatus("saving");
    setSaveMessage(t("modpack.modpackBuilder.saving"));
    try {
      if (format === "modrinth") {
        const deps: {
          minecraft: string;
          "fabric-loader"?: string;
          forge?: string;
          neoforge?: string;
          "quilt-loader"?: string;
        } = { minecraft: matchedVersion!.id };
        if (selectedLoader === "fabric") deps["fabric-loader"] = loaderVersion.trim();
        else if (selectedLoader === "forge") deps.forge = loaderVersion.trim();
        else if (selectedLoader === "neoforge") deps.neoforge = loaderVersion.trim();
        else if (selectedLoader === "quilt") deps["quilt-loader"] = loaderVersion.trim();

        await saveInstance({
          formatVersion: 1,
          game: "minecraft",
          versionId: packVersion.trim(),
          name: trimmed,
          summary: `RTLauncher Modrinth ${t("modpack.modpackBuilder.modpack")} - ${matchedVersion!.id}`,
          format: "modrinth",
          files: selectedModrinth.map((f) => ({
            path: f.path,
            hashes: f.hashes,
            env: f.env,
            downloads: f.downloads,
            fileSize: f.fileSize,
            display_name: f.display_name,
          })),
          dependencies: deps,
          loader: selectedLoader,
          loader_version: loaderVersion.trim(),
          author: author.trim() || undefined,
          optifine: useOptifine,
          optifine_version: useOptifine ? selectedOptifineVersion || null : null,
          cross_loader: crossLoader,
          created_at: 0,
          updated_at: 0,
        });
      } else {
        await saveInstance({
          format: "curseforge",
          name: trimmed,
          version: packVersion.trim(),
          author: author.trim(),
          game_version: matchedVersion!.id,
          loader: selectedLoader,
          loader_version: loaderVersion.trim(),
          optifine: useOptifine,
          optifine_version: useOptifine ? selectedOptifineVersion || null : null,
          cross_loader: crossLoader,
          created_at: 0,
          updated_at: 0,
          files: selectedCurseforge.map((f) => ({
            ...f,
            required: f.required !== undefined ? f.required : true,
          })),
        });
      }
      setSaveStatus("saved");
      setSaveMessage(`${t("modpack.modpackBuilder.saved")}${dir || "<minecraft>/modpack"}/${trimmed}.json`);
      setTimeout(() => {
        setSaveStatus((s) => (s === "saved" ? "idle" : s));
      }, 4000);
      return true;
    } catch (e: any) {
      setSaveStatus("error");
      setSaveMessage(`${t("modpack.modpackBuilder.saveFailed")}: ${e?.message || e}`);
      return false;
    }
  };

  const handleExport = async () => {
    if (exportBlockers.length > 0) {
      setSaveStatus("error");
      setSaveMessage(`导出前请处理：${exportBlockers.join("；")}`);
      return;
    }
    if (!(await handleSave(false))) return;

    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const extension = format === "modrinth" ? "mrpack" : "zip";
      const outputPath = await save({
        title: `导出 ${formatLabel}`,
        defaultPath: `${name.trim().replace(/[\\/:*?"<>|]/g, "_")}.${extension}`,
        filters: [
          {
            name: formatLabel,
            extensions: [extension],
          },
        ],
      });
      if (!outputPath) return;

      setSaveStatus("saving");
      setSaveMessage("正在生成标准整合包...");
      const exportedPath = await exportInstance(name.trim(), outputPath);
      setSaveStatus("saved");
      setSaveMessage(`导出完成：${exportedPath}`);
    } catch (e: any) {
      setSaveStatus("error");
      setSaveMessage(`导出失败: ${e?.message || e}`);
    }
  };

  // 点击"返回"按钮：先自动保存再跳转
  const handleBack = async () => {
    const canAutoSave =
      name.trim() &&
      mcVersionValid &&
      packVersion.trim() &&
      loaderVersion.trim() &&
      (format !== "curseforge" || author.trim());
    if (canAutoSave) {
      const saved = await handleSave(false);
      if (!saved) return;
    }
    router.push("/tools");
  };

  // ===========================================================================
  // 版本下拉候选（当用户输入时）
  // ===========================================================================
  const versionCandidates = useMemo(() => {
    if (!gameVer.trim()) {
      // 下拉框自身滚动，正式版、快照、愚人节版和远古版全部可直接选择。
      return mcVersions;
    }
    const q = gameVer.trim().toLowerCase();
    return mcVersions.filter((v) => v.id.toLowerCase().includes(q));
  }, [gameVer, mcVersions]);

  // ===========================================================================
  // 渲染
  // ===========================================================================

  const formatLabel = format === "modrinth" ? "Modrinth mrpack" : `CurseForge ${t("modpack.modpackBuilder.modpack")}`;
  const total =
    format === "modrinth" ? selectedModrinth.length : selectedCurseforge.length;
  const exportBlockers: string[] = [];
  if (!name.trim()) exportBlockers.push(L({ "zh-CN": "填写整合包名称", "en-US": "enter a modpack name" }));
  if (!packVersion.trim()) exportBlockers.push(L({ "zh-CN": "填写整合包版本", "en-US": "enter a modpack version" }));
  if (!loaderVersion.trim()) {
    exportBlockers.push(L({ "zh-CN": "填写加载器具体版本", "en-US": "select an exact loader version" }));
  } else if (loaderVersion.trim().toLowerCase() === "latest") {
    exportBlockers.push(L({ "zh-CN": "加载器版本填写具体版本号", "en-US": "replace latest with an exact loader version" }));
  }
  if (!mcVersionValid) exportBlockers.push(L({ "zh-CN": "选择有效的 Minecraft 版本", "en-US": "select a valid Minecraft version" }));
  if (format === "curseforge" && !author.trim()) exportBlockers.push(L({ "zh-CN": "填写作者", "en-US": "enter an author" }));
  if (format === "modrinth" && selectedLoader === "liteloader") {
    exportBlockers.push(L({ "zh-CN": "Modrinth 格式移除 LiteLoader", "en-US": "remove LiteLoader from the Modrinth format" }));
  }
  if (crossLoader) exportBlockers.push(L({ "zh-CN": "关闭信雅互联模式", "en-US": "disable cross-loader mode" }));
  if (useOptifine) exportBlockers.push(L({ "zh-CN": "关闭 OptiFine（文件未进入标准清单）", "en-US": "disable OptiFine because it is not in the standard manifest" }));
  if (total === 0) exportBlockers.push(L({ "zh-CN": "至少添加一个文件", "en-US": "add at least one file" }));

  return (
    <div className="flex flex-col h-full overflow-hidden bg-background">
      {/* 顶部标题栏 */}
      <div className="shrink-0 border-b border-border p-4">
        <div className="flex items-center gap-3">
          <Button
            variant="outline"
            size="sm"
            onClick={handleBack}
            className="gap-1"
          >
            <ArrowLeft className="size-4" />
            {t("common.back")}
          </Button>
          <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
            {format === "modrinth" ? (
              <Package className="size-5 text-primary" />
            ) : (
              <Box className="size-5 text-primary" />
            )}
          </div>
          <div className="flex-1">
            <div className="text-lg font-semibold leading-none">
              {t("modpack.modpackBuilder.create")} {formatLabel}
            </div>
            <div className="mt-1 text-xs text-muted-foreground">
              {t("modpack.modpackBuilder.totalFilesAddedClickBackToSaveTheInstance", { total: total })}
            </div>
          </div>
        </div>

        {/* 元数据输入 */}
        <div className="mt-4 grid grid-cols-1 md:grid-cols-4 gap-3">
          <div className="flex flex-col gap-1">
            <label className="text-xs text-muted-foreground">{t("modpack.modpackBuilder.modpackName")} *</label>
            <Input
              placeholder={t("modpack.modpackBuilder.eGMyFantasticPack")}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="flex flex-col gap-1">
            <label className="text-xs text-muted-foreground">整合包版本 *</label>
            <Input
              placeholder="如：1.0.0"
              value={packVersion}
              onChange={(e) => setPackVersion(e.target.value)}
            />
          </div>

          <div className="flex flex-col gap-1">
            <label className="text-xs text-muted-foreground">
              作者 {format === "curseforge" ? "*" : "（工程信息）"}
            </label>
            <Input
              placeholder="如：PlayerName"
              value={author}
              onChange={(e) => setAuthor(e.target.value)}
            />
          </div>

          {/* MC 版本：输入框 + 下拉候选 */}
          <div className="flex flex-col gap-1 relative">
            <label className="text-xs text-muted-foreground flex items-center gap-2">
              Minecraft {t("launch.versionSelector.version")} *
              {mcLoading && <Loader2 className="size-3 animate-spin" />}
              {mcError && <span className="text-red-500">· {t("modpack.modpackBuilder.loadFailed")}</span>}
            </label>
            <div className="relative">
              <Input
                placeholder={mcLoading ? t("modpack.modpackBuilder.loadingVersionList") : t("modpack.modpackBuilder.eG121124w10a")}
                value={gameVer}
                onChange={(e) => {
                  setGameVer(e.target.value);
                  setLoaderVersion("");
                  setVersionDropdownOpen(true);
                }}
                onFocus={() => setVersionDropdownOpen(true)}
                onBlur={() => {
                  // 延迟关闭，允许点击候选
                  setTimeout(() => setVersionDropdownOpen(false), 180);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Escape") setVersionDropdownOpen(false);
                  if (e.key === "Enter") setVersionDropdownOpen(false);
                }}
                className={`pr-10 ${
                  gameVer.trim() && !mcVersionValid
                    ? "border-red-400 focus-visible:ring-red-400"
                    : ""
                }`}
              />
              <button
                type="button"
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                onClick={() => setVersionDropdownOpen((v) => !v)}
                tabIndex={-1}
              >
                {gameVer.trim() && !mcVersionValid ? (
                  <X className="size-4 text-red-400" />
                ) : (
                  <ChevronDown className="size-4" />
                )}
              </button>

              {versionDropdownOpen && versionCandidates.length > 0 && (
                <div className="absolute left-0 right-0 top-full mt-1 bg-card border border-border rounded-lg shadow-xl z-40 max-h-64 overflow-y-auto">
                  {versionCandidates.map((v) => (
                    <button
                      type="button"
                      key={v.id}
                      className={`w-full text-left px-3 py-2 text-xs hover:bg-accent/60 flex items-center justify-between ${
                        matchedVersion?.id === v.id ? "bg-primary/10" : ""
                      }`}
                      onMouseDown={(e) => {
                        e.preventDefault();
                        setGameVer(v.id);
                        setLoaderVersion("");
                        setVersionDropdownOpen(false);
                      }}
                    >
                      <span className="font-mono">{v.id}</span>
                      <span className="flex items-center gap-2">
                        <Badge
                          variant={v.type === "release" ? "default" : "outline"}
                          className="text-[9px] py-0"
                        >
                          {v.type === "release"
                            ? t("modpack.modpackBuilder.release")
                            : v.type === "snapshot"
                              ? t("modpack.modpackBuilder.snapshot")
                              : v.type === "april_fools"
                                ? t("modpack.modpackBuilder.aprilFools")
                                : t("modpack.modpackBuilder.old")}
                        </Badge>
                        <span className="text-muted-foreground">{v.releaseDate}</span>
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
            {gameVer.trim() && !mcVersionValid && (
              <span className="text-[11px] text-red-500">
                {t("modpack.modpackBuilder.noMinecraftVersionMatchesSelectOneFromTheList")}
              </span>
            )}
            {mcVersionValid && (
              <span className="text-[11px] text-green-600 flex items-center gap-1">
                <CheckCircle2 className="size-3" />
                {t("modpack.modpackBuilder.matched")}{matchedVersion!.id} ({matchedVersion!.type})
              </span>
            )}
          </div>

          {/* 加载器：forge / neoforge / fabric / quilt / liteloader */}
          <div className="flex flex-col gap-1">
            <label className="text-xs text-muted-foreground flex items-center gap-2">
              {t("home.modLoader")} *
              {crossLoader && (
                <Badge variant="outline" className="text-[9px] py-0 text-amber-600 border-amber-400">
                  {t("modpack.modpackBuilder.crossLoader")}
                </Badge>
              )}
            </label>
            <div className="flex flex-wrap gap-1.5">
              {(format === "modrinth"
                ? ["forge", "neoforge", "fabric", "quilt"]
                : ["forge", "neoforge", "fabric", "quilt", "liteloader"]
              ).map((loader) => (
                <Badge
                  key={loader}
                  onClick={() => {
                    if (loader !== selectedLoader) setLoaderVersion("");
                    setSelectedLoader(loader);
                  }}
                  variant={selectedLoader === loader ? "default" : "outline"}
                  className={`cursor-pointer text-[11px] py-0.5 px-2 capitalize select-none ${crossLoader ? "opacity-60" : "hover:brightness-110"}`}
                >
                  {loader === "neoforge"
                    ? "NeoForge"
                    : loader.charAt(0).toUpperCase() + loader.slice(1)}
                </Badge>
              ))}
            </div>
            <Select value={loaderVersion} onValueChange={setLoaderVersion}>
              <SelectTrigger className="w-full" disabled={loaderVersionsLoading || !mcVersionValid}>
                <SelectValue
                  placeholder={
                    loaderVersionsLoading
                      ? "正在加载版本列表..."
                      : "点击选择加载器具体版本"
                  }
                />
              </SelectTrigger>
              <SelectContent className="max-h-72">
                {loaderVersionOptions.map((version) => (
                  <SelectItem key={version} value={version}>
                    {version}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {loaderVersionsError && (
              <span className="text-[11px] text-red-500 flex items-center gap-1">
                <AlertCircle className="size-3 shrink-0" /> 加载器版本加载失败：
                {loaderVersionsError}
              </span>
            )}
            <label className="text-[11px] text-muted-foreground flex items-center gap-2 mt-1">
              <input
                type="checkbox"
                checked={crossLoader}
                onChange={(e) => setCrossLoader(e.target.checked)}
                className="size-3 accent-primary"
              />
              <span>
                {t("modpack.modpackBuilder.enableCrossLoaderMode")}
              </span>
              <span className="text-[10px] text-amber-600">
                · {t("modpack.modpackBuilder.searchFabricAndForgeModsTogetherWithoutLoaderRestrictions")}
              </span>
            </label>
          </div>

          {/* OptiFine 勾选 + 版本选择 */}
          <div className="flex flex-col gap-2">
            <label className="text-xs text-muted-foreground flex items-center gap-2">
              <input
                type="checkbox"
                checked={useOptifine}
                onChange={(e) => {
                  setUseOptifine(e.target.checked);
                  if (!e.target.checked) setSelectedOptifineVersion("");
                }}
                className="size-3 accent-primary"
              />
              <span>{t("modpack.modpackBuilder.enableOptiFine")}</span>
              {optifineLoading && useOptifine && (
                <Loader2 className="size-3 animate-spin" />
              )}
            </label>
            {useOptifine && (
              <div className="flex flex-col gap-1">
                {optifineVersions.length === 0 && (
                  <span className="text-[11px] text-amber-600">
                    {t("modpack.modpackBuilder.noOptiFineVersionsAreCurrentlyAvailableForId", { id: matchedVersion?.id ?? "?" })}
                  </span>
                )}
                {optifineVersions.length > 0 && (
                  <div className="flex flex-wrap gap-1.5">
                    {optifineVersions.map((v) => (
                      <Badge
                        key={v.filename}
                        onClick={() => setSelectedOptifineVersion(v.filename)}
                        variant={selectedOptifineVersion === v.filename ? "default" : "outline"}
                        className="cursor-pointer text-[11px] py-0.5 px-2 hover:brightness-110"
                        title={`${v.id} · ${t("modpack.modpackBuilder.type")}${v.type_}`}
                      >
                        {v.patch || v.id}
                      </Badge>
                    ))}
                  </div>
                )}
                {useOptifine && optifineVersions.length > 0 && !selectedOptifineVersion && (
                  <span className="text-[11px] text-amber-600">{t("modpack.modpackBuilder.selectAnOptiFineVersion")}</span>
                )}
              </div>
            )}
          </div>

          <div className="flex flex-col items-end justify-end gap-1">
            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={handleExport}
                disabled={saveStatus === "saving"}
                className="gap-1"
              >
                <Download className="size-4" />
                {L({ "zh-CN": "导出标准包", "en-US": "Export standard pack" })}
              </Button>
              <Button
                size="sm"
                onClick={() => handleSave(false)}
                disabled={
                  !name.trim() ||
                  !packVersion.trim() ||
                  !loaderVersion.trim() ||
                  !mcVersionValid ||
                  (format === "curseforge" && !author.trim()) ||
                  saveStatus === "saving" ||
                  (useOptifine && optifineVersions.length > 0 && !selectedOptifineVersion)
                }
                className="gap-1"
              >
                {saveStatus === "saving" ? (
                  <Loader2 className="size-4 animate-spin" />
                ) : saveStatus === "saved" ? (
                  <CheckCircle2 className="size-4 text-green-500" />
                ) : saveStatus === "error" ? (
                  <AlertCircle className="size-4 text-red-500" />
                ) : (
                  <Save className="size-4" />
                )}
                {L({ "zh-CN": "手动保存", "en-US": "Save" })}
              </Button>
            </div>
            {exportBlockers.length > 0 && (
              <div className="max-w-[32rem] text-right text-[11px] text-amber-600">
                {L({ "zh-CN": "导出前：", "en-US": "Before export: " })}{exportBlockers.join(L({ "zh-CN": "；", "en-US": "; " }))}
              </div>
            )}
          </div>
        </div>

        {/* 保存状态 */}
        {saveStatus !== "idle" && (
          <div
            className={`mt-2 text-xs ${
              saveStatus === "error"
                ? "text-red-500"
                : saveStatus === "saved"
                  ? "text-green-600"
                  : "text-muted-foreground"
            }`}
          >
            {saveMessage}
          </div>
        )}

        {dir && (
          <div className="mt-2 text-xs text-muted-foreground flex items-center gap-1">
            <Folder className="size-3" />
            {t("modpack.modpackBuilder.savedIn")}<span className="font-mono">{dir}</span>
          </div>
        )}
      </div>

      {/* 主体内容 */}
      <div className="flex-1 overflow-hidden grid grid-cols-1 lg:grid-cols-5 gap-0">
        {/* 左：搜索 + 项目详情 */}
        <div className="lg:col-span-3 overflow-y-auto border-r border-border p-4 space-y-4">
          {/* 搜索栏 */}
          <div>
            <div className="text-xs text-muted-foreground mb-2">{t("modpack.modpackBuilder.chooseAResourceCategory")}</div>
            <div className="flex flex-wrap gap-2 mb-3">
              {(
                [
                  ["mod", "Mods"],
                  ["modpack", "Modpacks"],
                  ["resourcepack", "Resource Packs"],
                  ["shaders", "Shaders"],
                  ["datapack", "Data Packs"],
                  ["worlds", "Worlds"],
                ] as [CategoryId, string][]
              ).map(([id, label]) => (
                <Button
                  key={id}
                  variant={category === id ? "default" : "outline"}
                  size="sm"
                  onClick={() => setCategory(id)}
                >
                  {label}
                </Button>
              ))}
            </div>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
                <Input
                  placeholder={
                    !mcVersionValid
                      ? t("modpack.modpackBuilder.enterAMinecraftVersionAboveFirst")
                      : format === "modrinth"
                        ? t("modpack.modpackBuilder.searchModrinthProjectsMcValue", { value: matchedVersion?.id || "?" })
                        : t("modpack.modpackBuilder.searchCurseForgeProjectsMcValue", { value: matchedVersion?.id || "?" })
                  }
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && doSearch()}
                  disabled={!mcVersionValid}
                  className="pl-9"
                />
              </div>
              <Button onClick={doSearch} disabled={searchLoading || !mcVersionValid}>
                {searchLoading ? <Loader2 className="size-4 animate-spin mr-1" /> : null}
                {t("modpack.modpackBuilder.search")}
              </Button>
            </div>
            {searchError && (
              <div className="mt-2 text-xs text-red-500 flex items-center gap-1">
                <AlertCircle className="size-3" /> {searchError}
              </div>
            )}
          </div>

          {/* 搜索结果列表 */}
          {results !== null && (
            <div className="rounded-xl border border-border overflow-hidden">
              <div className="px-3 py-2 border-b bg-muted/30 text-xs text-muted-foreground">
                {results.length} {t("modpack.modpackBuilder.results")} (MC {matchedVersion?.id})
              </div>
              <div className="divide-y divide-border max-h-[35vh] overflow-y-auto">
                {results.map((hit, idx) => (
                  <button
                    key={idx}
                    onClick={() => openHit(hit)}
                    className={`w-full text-left p-3 hover:bg-accent/50 transition-colors flex items-center gap-3 ${
                      activeHit?.slug === hit.slug && activeHit?.source === hit.source
                        ? "bg-accent/40"
                        : ""
                    }`}
                  >
                    {hit.iconUrl ? (
                      // eslint-disable-next-line @next/next/no-img-element
                      <img
                        src={hit.iconUrl}
                        alt=""
                        className="w-10 h-10 rounded-lg object-cover bg-muted shrink-0"
                        onError={(e) => {
                          (e.currentTarget as HTMLImageElement).style.display = "none";
                        }}
                      />
                    ) : (
                      <div className="w-10 h-10 rounded-lg bg-muted shrink-0 flex items-center justify-center">
                        <Box className="size-4 text-muted-foreground" />
                      </div>
                    )}
                    <div className="min-w-0 flex-1">
                      <div className="font-medium text-sm truncate">{hit.title}</div>
                      <div className="text-xs text-muted-foreground truncate">
                        {hit.description || "—"}
                      </div>
                      <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground flex-wrap">
                        <Badge
                          variant={hit.source === "modrinth" ? "default" : "outline"}
                          className="text-[10px] py-0"
                        >
                          {hit.source}
                        </Badge>
                        <Badge variant="outline" className="text-[10px] py-0">
                          {category === "modpack"
                            ? `支持 ${hit.game_versions?.join(", ") || "未知 MC 版本"}`
                            : `MC ${matchedVersion?.id}`}
                        </Badge>
                        <span>{hit.downloads?.toLocaleString()} {t("modpack.modpackBuilder.downloads")}</span>
                        {hit.author && <span>· {hit.author}</span>}
                      </div>
                    </div>
                    <span className="shrink-0 text-xs font-medium text-primary flex items-center gap-1">
                      选择版本 <ChevronDown className="size-3.5" />
                    </span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* 项目详情 + 文件选择 */}
          {activeHit && (
            <div ref={detailsRef} className="rounded-xl border border-border p-4 scroll-mt-4">
              <div className="flex items-center justify-between mb-3">
                <div className="font-medium text-sm flex items-center gap-2">
                  <Package className="size-4 text-primary" /> {activeHit.title}
                </div>
                <Badge variant="outline" className="text-[10px]">
                  {activeHit.source}
                </Badge>
              </div>
              <div className="text-xs text-muted-foreground mb-3">
                在下面选择需要的版本，然后点击右侧“添加”。
              </div>
              {addError && (
                <div className="mb-3 rounded-md border border-red-500/30 bg-red-500/5 px-3 py-2 text-xs text-red-500 flex items-center gap-1">
                  <AlertCircle className="size-3 shrink-0" /> {addError}
                </div>
              )}

              {activeLoading ? (
                <div className="py-4 flex items-center gap-2 text-xs text-muted-foreground">
                  <Loader2 className="size-4 animate-spin" /> {t("modpack.modpackBuilder.loadingFileInformation")}
                </div>
              ) : activeError ? (
                <div className="py-2 text-xs text-red-500 flex items-center gap-1">
                  <AlertCircle className="size-3" /> {activeError}
                </div>
              ) : format === "modrinth" ? (
                <div className="space-y-2 max-h-[38vh] overflow-y-auto">
                  {modrinthVersions.length === 0 ? (
                    <div className="text-xs text-muted-foreground py-2">
                      {category === "modpack"
                        ? L({ "zh-CN": "该项目暂时没有可下载版本", "en-US": "This project currently has no downloadable versions" })
                        : L({ "zh-CN": `无可用版本（该项目没有适配 MC ${matchedVersion?.id}）`, "en-US": `No versions are available for MC ${matchedVersion?.id}` })}
                    </div>
                  ) : (
                    modrinthVersions.map((version) => {
                      const primary = version.files[0];
                      const isAdding = addingFileKey === `modrinth:${version.id}`;
                      const already = selectedModrinth.some(
                        (file) => file.hashes.sha1 === primary?.hashes.sha1,
                      );
                      return (
                        <div
                          key={version.id}
                          className="border border-border rounded-lg p-3 hover:border-primary/50 transition-colors"
                        >
                          <div className="flex items-center justify-between gap-3">
                            <div className="min-w-0">
                              <div className="font-medium text-sm truncate">
                                {version.version_number || version.id}
                              </div>
                              <div className="text-xs text-muted-foreground flex flex-wrap items-center gap-1">
                                {version.loaders.length > 0 && (
                                  <Badge variant="outline" className="text-[10px]">
                                    {version.loaders.join(", ")}
                                  </Badge>
                                )}
                                <Badge variant="outline" className="text-[10px]">
                                  MC {version.game_versions.join(", ") || "未知"}
                                </Badge>
                                <span className="text-[10px]">
                                  {(primary?.size || 0) > 0
                                    ? `${(primary!.size / 1024).toFixed(1)} KB`
                                    : ""}
                                </span>
                                <span className="text-[10px]">· {version.version_type}</span>
                              </div>
                            </div>
                            <Button
                              size="sm"
                              onClick={() => addModrinthFile(activeHit, version)}
                              disabled={already || addingFileKey !== null}
                            >
                              {isAdding ? (
                                <>
                                  <Loader2 className="size-4 mr-1 animate-spin" /> 添加依赖中
                                </>
                              ) : already ? (
                                <>
                                  <CheckCircle2 className="size-4 mr-1 text-green-500" />
                                  {t("modpack.modpackBuilder.added")}
                                </>
                              ) : (
                                <>
                                  <Plus className="size-4 mr-1" /> {t("common.add")}
                                </>
                              )}
                            </Button>
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              ) : (
                <div className="space-y-2 max-h-[38vh] overflow-y-auto">
                  {curseforgeFiles.length === 0 ? (
                    <div className="text-xs text-muted-foreground py-2">
                      {category === "modpack"
                        ? L({ "zh-CN": "该项目暂时没有可下载文件", "en-US": "This project currently has no downloadable files" })
                        : L({ "zh-CN": `无可用文件（该项目没有适配 MC ${matchedVersion?.id}）`, "en-US": `No files are available for MC ${matchedVersion?.id}` })}
                    </div>
                  ) : (
                    curseforgeFiles.map((file) => {
                      const isAdding = addingFileKey === `curseforge:${file.id}`;
                      const already = selectedCurseforge.some(
                        (selected) =>
                          selected.projectID === curseforgeProjectId &&
                          selected.fileID === file.id,
                      );
                      return (
                        <div
                          key={file.id}
                          className="border border-border rounded-lg p-3 hover:border-primary/50 transition-colors"
                        >
                          <div className="flex items-center justify-between gap-3">
                            <div className="min-w-0">
                              <div className="font-medium text-sm truncate">
                                {file.displayName || file.fileName || `File #${file.id}`}
                              </div>
                              <div className="text-xs text-muted-foreground flex flex-wrap items-center gap-1">
                                <Badge variant="outline" className="text-[10px]">
                                  MC {(file.gameVersions || []).join(", ") || "未知"}
                                </Badge>
                                <Badge variant="outline" className="text-[10px]">
                                  {file.releaseType === 1
                                    ? L({ "zh-CN": "正式版", "en-US": "Release" })
                                    : file.releaseType === 2
                                      ? "Beta"
                                      : "Alpha"}
                                </Badge>
                                {file.fileDate && (
                                  <span className="text-[10px]">
                                    · {formatTimestamp(
                                      Math.floor(new Date(file.fileDate).getTime() / 1000),
                                    )}
                                  </span>
                                )}
                              </div>
                            </div>
                            <Button
                              size="sm"
                              onClick={() => addCurseforgeFile(file)}
                              disabled={already || addingFileKey !== null}
                            >
                              {isAdding ? (
                                <>
                                  <Loader2 className="size-4 mr-1 animate-spin" /> 添加依赖中
                                </>
                              ) : already ? (
                                <>
                                  <CheckCircle2 className="size-4 mr-1 text-green-500" />
                                  {t("modpack.modpackBuilder.added")}
                                </>
                              ) : (
                                <>
                                  <Plus className="size-4 mr-1" /> {t("common.add")}
                                </>
                              )}
                            </Button>
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              )}
            </div>
          )}
        </div>

        {/* 右：已选文件 */}
        <div className="lg:col-span-2 overflow-y-auto p-4 bg-muted/20">
          <div className="font-medium text-sm mb-2 flex items-center gap-2">
            <Box className="size-4 text-primary" />
            {t("modpack.modpackBuilder.added")} ({total})
          </div>
          <div className="space-y-2">
            {format === "modrinth" ? (
              selectedModrinth.length === 0 ? (
                <div className="text-xs text-muted-foreground py-6 text-center border border-dashed rounded-xl">
                  {t("modpack.modpackBuilder.noFilesYetSearchAndAddFilesFromThe")}
                </div>
              ) : (
                selectedModrinth.map((f, idx) => (
                  <div
                    key={`${f.path}-${f.hashes.sha1}`}
                    className="bg-card border border-border rounded-xl p-3"
                  >
                    <div className="flex items-start justify-between gap-2 mb-2">
                      <div className="min-w-0 flex-1">
                        <div className="font-medium text-sm truncate">
                          {f.display_name || f.path.split("/").pop()}
                        </div>
                        <div className="text-xs text-muted-foreground font-mono truncate">
                          {f.path}
                        </div>
                      </div>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => removeModrinth(idx)}
                      >
                        <Trash2 className="size-3.5 text-red-500" />
                      </Button>
                    </div>
                    <div className="grid grid-cols-2 gap-2 text-xs">
                      <div className="flex items-center gap-2 rounded-md border border-border bg-muted/30 px-2 py-1.5">
                        <Monitor className="size-3 text-muted-foreground" />
                        <span>{L({ "zh-CN": "客户端：", "en-US": "Client: " })}{environmentRequirementLabel(f.env.client)}</span>
                      </div>
                      <div className="flex items-center gap-2 rounded-md border border-border bg-muted/30 px-2 py-1.5">
                        <Server className="size-3 text-muted-foreground" />
                        <span>{L({ "zh-CN": "服务端：", "en-US": "Server: " })}{environmentRequirementLabel(f.env.server)}</span>
                      </div>
                    </div>
                    <div className="mt-2 text-[10px] text-muted-foreground font-mono truncate">
                      sha1: {f.hashes.sha1.slice(0, 16)}...
                    </div>
                  </div>
                ))
              )
            ) : selectedCurseforge.length === 0 ? (
              <div className="text-xs text-muted-foreground py-6 text-center border border-dashed rounded-xl">
                {t("modpack.modpackBuilder.noFilesYetSearchAndAddFilesFromThe")}
              </div>
            ) : (
              selectedCurseforge.map((f, idx) => (
                <div
                  key={`${f.projectID}-${f.fileID}`}
                  className="bg-card border border-border rounded-xl p-3"
                >
                  <div className="flex items-start justify-between gap-2 mb-2">
                    <div className="min-w-0 flex-1">
                      <div className="font-medium text-sm truncate">
                        {f.display_name || `Project #${f.projectID} File #${f.fileID}`}
                      </div>
                      <div className="text-xs text-muted-foreground font-mono">
                        projectID: {f.projectID} · fileID: {f.fileID}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => removeCurseforge(idx)}
                    >
                      <Trash2 className="size-3.5 text-red-500" />
                    </Button>
                  </div>
                  <div className="flex items-center gap-2 text-xs">
                    <label className="flex items-center gap-2 cursor-pointer select-none">
                      <input
                        type="checkbox"
                        className="accent-primary"
                        checked={f.required !== false}
                        onChange={() => toggleCurseforgeRequired(idx)}
                      />
                      <span>
                        {f.required !== false ? t("modpack.modpackBuilder.required") : t("modpack.modpackBuilder.optional")}
                      </span>
                    </label>
                  </div>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}