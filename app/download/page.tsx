"use client";

import { useState, useMemo, useEffect, useCallback } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Download, Loader2, AlertCircle, RefreshCw, Coffee, Search, ArrowRight, ExternalLink, Box, Palette, Sparkles, Database, Boxes, Map, Package } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { VersionList } from "@/components/download/version-list";
import { VersionDetail } from "@/components/download/version-detail";
import {
  VersionFilterBar,
} from "@/components/download/version-filter-bar";
import { useMinecraftVersions } from "@/hooks/use-minecraft-versions";
import { slideInFromRight, slideInFromLeft, fadeIn } from "@/lib/motion";
import { useDownloadManager } from "@/components/download/download-provider";
import { invoke } from "@tauri-apps/api/core";
import { useRouter } from "next/navigation";
import type { MinecraftVersion, MinecraftVersionType } from "@/types";

// -------- English search --------
type EnglishCategory = "mod" | "modpack" | "resourcepack" | "shaders" | "datapack" | "worlds";

const ENGLISH_CATEGORIES: { id: EnglishCategory; label: string; short: string; icon: typeof Box }[] = [
  { id: "mod", label: "Mods", short: "Mods", icon: Box },
  { id: "modpack", label: "Modpacks", short: "Modpacks", icon: Boxes },
  { id: "resourcepack", label: "Resource Packs", short: "Resource Packs", icon: Palette },
  { id: "shaders", label: "Shader Packs", short: "Shaders", icon: Sparkles },
  { id: "datapack", label: "Data Packs", short: "Data Packs", icon: Database },
  { id: "worlds", label: "Worlds", short: "Worlds", icon: Map },
];

interface SearchResultItem {
  title: string;
  slug: string;
  description?: string;
  iconUrl?: string;
  source: "modrinth" | "curseforge" | "both";
  projectType: string;
  downloads?: number;
  categories?: string[];
  latestVersions?: string[];
  mcVersions?: string[];
  updated?: string;
  author?: string;
  externalUrl?: string;
}

function formatNumber(n?: number): string {
  if (n === undefined || n === null) return "";
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1).replace(/\.0$/, "") + "M";
  if (n >= 1_000) return (n / 1_000).toFixed(1).replace(/\.0$/, "") + "K";
  return String(n);
}

function formatDate(iso?: string): string {
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

function categoryPath(category: string, slug: string, returnQuery?: string, returnCategory?: EnglishCategory): string {
  // Route to the detail page and keep enough state to restore English search.
  const params = new URLSearchParams({ mod: slug });
  if (returnQuery !== undefined && returnCategory !== undefined) {
    params.set("returnTo", "english");
    params.set("query", returnQuery);
    params.set("category", returnCategory);
  }
  return `/download/detail?${params.toString()}`;
}

function categoryForSource(projectType: string): string {
  const map: Record<string, string> = {
    mod: "mod",
    modpack: "modpack",
    resourcepack: "resourcepack",
    "resource pack": "resourcepack",
    shader: "shaders",
    shaders: "shaders",
    "shader pack": "shaders",
    datapack: "datapack",
    "data pack": "datapack",
    worlds: "worlds",
    world: "worlds",
  };
  return map[projectType.toLowerCase()] ?? projectType;
}

function compareMinecraftVersionsDescending(a: MinecraftVersion, b: MinecraftVersion): number {
  const parts = (version: string) => {
    const match = version.match(/^(\d+(?:\.\d+)+)/);
    return match ? match[1].split(".").map(Number) : null;
  };
  const aParts = parts(a.id);
  const bParts = parts(b.id);
  if (aParts && bParts) {
    const length = Math.max(aParts.length, bParts.length);
    for (let index = 0; index < length; index += 1) {
      const difference = (bParts[index] ?? 0) - (aParts[index] ?? 0);
      if (difference !== 0) return difference;
    }
  }
  return new Date(b.releaseDate).getTime() - new Date(a.releaseDate).getTime();
}

/**
 * Download Center
 * Contains Minecraft version downloads, Java downloads, and Chinese mod search
 */
export default function DownloadPage() {
  const router = useRouter();
  const { versions, loading, error, refetch } = useMinecraftVersions();
  const [versionFilter, setVersionFilter] = useState<MinecraftVersionType>("release");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedVersion, setSelectedVersion] =
    useState<MinecraftVersion | null>(null);

  const [tab, setTab] = useState<"minecraft" | "java" | "chinese" | "english">("minecraft");

  // Java 下载状态
  const [javaVersions, setJavaVersions] = useState<{ name: string; version: string }[]>([]);
  const [javaVersionsLoading, setJavaVersionsLoading] = useState(false);
  const [javaMessage, setJavaMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [javaVersionFilter, setJavaVersionFilter] = useState<"all" | "jre" | "jdk">("all");
  const [javaSearchQuery, setJavaSearchQuery] = useState("");

  // 中文检索状态
  const [chineseSearchQuery, setChineseSearchQuery] = useState("");
  const [chineseSearchResults, setChineseSearchResults] = useState<
    { slug: string; chinese_name: string; mcmod_id?: number }[] | null
  >(null);
  const [chineseSearchLoading, setChineseSearchLoading] = useState(false);
  const [chineseSearchError, setChineseSearchError] = useState<string | null>(null);

  // 英文检索状态
  const [englishQuery, setEnglishQuery] = useState("");
  const [englishCategory, setEnglishCategory] = useState<EnglishCategory>("mod");
  const [englishResults, setEnglishResults] = useState<SearchResultItem[] | null>(null);
  const [englishLoading, setEnglishLoading] = useState(false);
  const [englishError, setEnglishError] = useState<string | null>(null);
  const [englishSourceInfo, setEnglishSourceInfo] = useState<
    { modrinth: { ok: boolean; count: number; error?: string }; curseforge: { ok: boolean; count: number; error?: string } } | null
  >(null);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    if (params.get("tab") !== "english") return;

    const category = params.get("category");
    setTab("english");
    setEnglishQuery(params.get("query") || "");
    if (category && ENGLISH_CATEGORIES.some((item) => item.id === category)) {
      setEnglishCategory(category as EnglishCategory);
    }
  }, []);

  // 检查字符串是否包含中文字符（用于判断输入）
  const hasChinese = (text: string): boolean => {
    // Unicode 范围：基本汉字区 + 扩展 A + 扩展 B + 常见标点
    const chineseRegex = /[\u4e00-\u9fff\u3400-\u4dbf\u20000-\u2a6df\u3000-\u303f\uff00-\uffef]/;
    return chineseRegex.test(text);
  };

  // 检查字符串是否全为标点/空白/非实质内容
  const isOnlyPunctuationOrEmpty = (text: string): boolean => {
    const trimmed = text.trim();
    if (trimmed.length === 0) return true;
    // 仅包含常见标点符号、空白、数字
    const nonContentRegex = /^[\s\d\p{P}\p{S}]+$/u;
    return nonContentRegex.test(trimmed);
  };

  const handleChineseSearch = async () => {
    const query = chineseSearchQuery.trim();

    setChineseSearchLoading(true);
    setChineseSearchResults(null);
    setChineseSearchError(null);

    try {
      // 1) Empty content check
      if (query.length === 0) {
        setChineseSearchError("Please enter search keywords");
        return;
      }

      // 2) Punctuation/numbers only check
      if (isOnlyPunctuationOrEmpty(query)) {
        setChineseSearchError("Please enter meaningful Chinese keywords");
        return;
      }

      // 3) Non-Chinese check — must contain at least one Chinese character
      if (!hasChinese(query)) {
        setChineseSearchError("Please use Chinese keywords for search");
        return;
      }

      // 4) Call backend SQLite query
      const result = await invoke<string>("search_moddata", { keyword: query });
      const parsed = JSON.parse(result) as { slug: string; chinese_name: string }[];
      setChineseSearchResults(parsed);

      if (parsed.length === 0) {
        setChineseSearchError(`No mods found related to "${query}"`);
      }
    } catch (err) {
      console.error("Chinese search failed:", err);
      setChineseSearchError(`Search failed: ${String(err)}`);
    } finally {
      setChineseSearchLoading(false);
    }
  };

  // Navigate to mod detail page when clicking result item
  const handleResultClick = (item: { slug: string; chinese_name: string }) => {
    router.push(`/download/detail?mod=${encodeURIComponent(item.slug)}&forceType=mod&returnTo=chinese`);
  };

  // English search handling
  const handleEnglishSearch = useCallback(async () => {
    const query = englishQuery.trim();
    if (!query) {
      setEnglishError("Please enter search keywords");
      return;
    }
    setEnglishLoading(true);
    setEnglishResults(null);
    setEnglishError(null);
    setEnglishSourceInfo(null);
    try {
      // Map englishCategory to Modrinth project_type / CurseForge classId
      const modrinthProjectType = englishCategory === "shaders" ? "shader" : englishCategory === "worlds" ? "world" : englishCategory === "modpack" ? "modpack" : englishCategory;
      // Infer CurseForge classId based on englishCategory
      const targetClassIds: number[] = (() => {
        switch (englishCategory) {
          case "modpack": return [4471, 4473];
          case "resourcepack": return [12];
          case "shaders": return [6552];
          case "datapack": return [6945, 6949];
          case "worlds": return [17];
          default: return [6];
        }
      })();
      const isModCategory = englishCategory === "mod";
      // Project type to mark in search results (for detail page to distinguish mod/modpack/resourcepack etc.)
      const resultProjectType = (() => {
        switch (englishCategory) {
          case "modpack": return "modpack";
          case "resourcepack": return "resourcepack";
          case "shaders": return "shaderpack";
          case "datapack": return "datapack";
          case "worlds": return "world";
          default: return "mod";
        }
      })();
      // Infer correct URL path part from CurseForge project classId
      const classIdToCfPath = (classId: number | undefined, slug: string): string => {
        switch (classId) {
          case 6: return `https://www.curseforge.com/minecraft/mc-mods/${slug}`;
          case 4471: return `https://www.curseforge.com/minecraft/modpacks/${slug}`;
          case 4473: return `https://www.curseforge.com/minecraft/modpacks/${slug}`;
          case 12: return `https://www.curseforge.com/minecraft/texture-packs/${slug}`;
          case 6552: return `https://www.curseforge.com/minecraft/shaders/${slug}`;
          case 6945: return `https://www.curseforge.com/minecraft/data-packs/${slug}`;
          case 6949: return `https://www.curseforge.com/minecraft/data-packs/${slug}`;
          case 17: return `https://www.curseforge.com/minecraft/worlds/${slug}`;
          default: return `https://www.curseforge.com/minecraft/mc-mods/${slug}`;
        }
      };
      // Use the native client: direct WebView requests can be blocked by CORS
      // or local proxy policy even while Modrinth itself is reachable.
      const modrinthPromise = invoke<string>('search_modrinth_projects', {
        query,
        projectType: modrinthProjectType,
        limit: 25,
      }).then(result => {
        try {
          return JSON.parse(result);
        } catch {
          return null;
        }
      }).catch((err) => {
        console.warn('Modrinth search failed:', err);
        return null;
      });

      // CurseForge searches through backend proxy (avoid CORS issues, improve classId search strategy)
      const cfPromise = invoke('search_curseforge_projects', {
        query: query,
        category: englishCategory,
        pageSize: 50
      }).then(result => {
        if (typeof result === 'string') {
          try {
            return JSON.parse(result);
          } catch {
            return null;
          }
        }
        return result;
      }).catch((err) => {
        console.warn('CurseForge search failed:', err);
        return null;
      });

      const [modrinthData, cfData] = await Promise.all([modrinthPromise, cfPromise]);

      // Get filtered CurseForge projects directly from backend proxy results
      const cfAllItems: any[] = Array.isArray(cfData?.data) ? cfData.data : [];
      const seenCfIds = new Set<string>();
      // Deduplicate results (backend proxy has done main filtering, but ensure frontend also deduplicates)
      const finalCfItems = cfAllItems.filter((mod: any) => {
        const cfId = String(mod.id);
        if (seenCfIds.has(cfId)) return false;
        seenCfIds.add(cfId);
        return true;
      });

      const mergedResults: SearchResultItem[] = [];
      const seenSlugs = new Set<string>();

      if (modrinthData?.hits) {
        for (const hit of modrinthData.hits) {
          const slug = hit.slug || hit.project_id || hit.id;
          if (seenSlugs.has(slug)) continue;
          seenSlugs.add(slug);
          mergedResults.push({
            slug,
            title: hit.title,
            description: hit.description,
            iconUrl: hit.icon_url,
            projectType: hit.project_type,
            downloads: hit.downloads,
            categories: hit.categories,
            latestVersions: hit.versions,
            mcVersions: hit.game_versions,
            updated: hit.date_modified || hit.date_updated,
            author: hit.author,
            externalUrl: `https://modrinth.com/${hit.project_type || "mod"}/${slug}`,
            source: "modrinth",
          });
        }
      }

      if (finalCfItems.length > 0) {
        for (const mod of finalCfItems) {
          const slug = mod.slug || String(mod.id);
          if (seenSlugs.has(slug)) {
            // Already from Modrinth, mark as dual source
            const existing = mergedResults.find((r) => r.slug === slug);
            if (existing) existing.source = "both";
            continue;
          }
          seenSlugs.add(slug);
          const logoUrl = mod.logo?.thumbnailUrl || mod.logo?.url;
          const authors = mod.authors?.map((a: any) => a.name).join(", ");
          const updated = mod.dateModified || mod.dateReleased;
          const links: string[] = [];
          if (mod.links?.websiteUrl) links.push(mod.links.websiteUrl);
          const fileIdx = mod.latestFilesIndexes || [];
          const gameVersions = fileIdx.map((f: any) => f.gameVersion).filter(Boolean);
          mergedResults.push({
            slug,
            title: mod.name,
            description: mod.summary,
            iconUrl: logoUrl,
            projectType: resultProjectType,
            downloads: mod.downloadCount,
            categories: [],
            latestVersions: [],
            mcVersions: gameVersions,
            updated,
            author: authors,
            externalUrl: links[0] || classIdToCfPath(mod.classId, slug),
            source: "curseforge",
          });
        }
      }

      // Sort by download count descending (ensure popular projects come first)
      mergedResults.sort((a, b) => (b.downloads || 0) - (a.downloads || 0));

      setEnglishResults(mergedResults);
      // CurseForge statistics: check if backend proxy returned valid data
      const cfHasSuccess = cfData?.data && Array.isArray(cfData.data);
      setEnglishSourceInfo({
        modrinth: modrinthData ? { ok: true, count: modrinthData.hits?.length || 0 } : { ok: false, count: 0, error: "No data returned" },
        curseforge: cfHasSuccess ? { ok: true, count: finalCfItems.length } : { ok: false, count: 0, error: "No data returned" },
      });

      if (mergedResults.length === 0) {
        setEnglishError(`No results found for "${query}" in ${ENGLISH_CATEGORIES.find((c) => c.id === englishCategory)?.label ?? englishCategory}`);
      }
    } catch (err) {
      console.error("English search failed:", err);
      setEnglishError(`Search failed: ${String(err)}`);
    } finally {
      setEnglishLoading(false);
    }
  }, [englishQuery, englishCategory]);

  // When switching to english tab or changing category, re-run search if there is an active query
  useEffect(() => {
    if (tab === "english" && englishQuery.trim()) {
      handleEnglishSearch();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab, englishCategory]);

  // Use download manager to track Java download tasks
  const { startJavaDownload, cancelDownload, tasks } = useDownloadManager();

  useEffect(() => {
    invoke<string>("get_java_download_dir").then(setJavaBasePath).catch(() => {});
  }, []);
  const [javaBasePath, setJavaBasePath] = useState("");

  const loadJavaVersions = async () => {
    setJavaVersionsLoading(true);
    try {
      const result = await invoke<{ name: string; version: string }[]>("get_java_versions");
      setJavaVersions(result);
    } catch (err) {
      setJavaMessage({ type: "error", text: `Failed to get Java versions: ${err}` });
    } finally {
      setJavaVersionsLoading(false);
    }
  };

  // Load version list when switching to Java tab
  useEffect(() => {
    if (tab === "java" && javaVersions.length === 0 && !javaVersionsLoading) {
      loadJavaVersions();
    }
  }, [tab]);

  const handleJavaDownload = async (runtimeName: string) => {
    if (!javaBasePath) {
      setJavaMessage({ type: "error", text: "Download path not ready, please try again later" });
      return;
    }
    setJavaMessage(null);
    try {
      const taskId = await startJavaDownload(runtimeName);
      const result = await invoke<{ message: string; java_path: string }>("download_java_runtime", {
        runtimeName,
        basePath: javaBasePath,
        taskId,
      });
      setJavaMessage({ type: "success", text: result.message });
      try {
        await invoke("validate_java_path", { javaPath: result.java_path });
      } catch { /* ignore */ }
    } catch (err) {
      setJavaMessage({ type: "error", text: `Download failed: ${err}` });
    }
  };

  // Filter Java versions
  const filteredJavaVersions = useMemo(() => {
    return javaVersions.filter((v) => {
      if (javaSearchQuery &&
          !v.name.toLowerCase().includes(javaSearchQuery.toLowerCase()) &&
          !v.version.toLowerCase().includes(javaSearchQuery.toLowerCase())) {
        return false;
      }
      if (javaVersionFilter !== "all") {
        const isJre = v.name.toLowerCase().includes("jre") ||
                    v.name.toLowerCase().includes("jfx");
        if (javaVersionFilter === "jre" && !isJre) return false;
        if (javaVersionFilter === "jdk" && isJre) return false;
      }
      return true;
    });
  }, [javaVersions, javaVersionFilter, javaSearchQuery]);

  const filteredVersions = useMemo(() => {
    return versions.filter((v) => {
      if (v.type !== versionFilter) return false;
      if (
        searchQuery &&
        !v.id.toLowerCase().includes(searchQuery.toLowerCase())
      )
        return false;
      return true;
    }).sort(compareMinecraftVersionsDescending);
  }, [versions, versionFilter, searchQuery]);

  return (
    <>
    <AnimatePresence mode="wait" initial={false}>
      {selectedVersion ? (
        // 版本详情视图
        <motion.div
          key="detail"
          className="flex h-full flex-col p-4"
          variants={slideInFromRight}
          initial="initial"
          animate="animate"
          exit="exit"
        >
          <VersionDetail
            version={selectedVersion}
            onBack={() => setSelectedVersion(null)}
          />
        </motion.div>
      ) : (
        // 列表视图
        <motion.div
          key="list"
          className="flex h-full flex-col gap-4 p-4"
          variants={slideInFromLeft}
          initial="initial"
          animate="animate"
          exit="exit"
        >
          {/* 页面标题 */}
          <div className="flex items-center justify-between gap-3 shrink-0">
            <div className="flex items-center gap-3">
              <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
                <Download className="size-5 text-primary" />
              </div>
              <div>
                <h1 className="text-lg font-semibold leading-none">Download Center</h1>
                <p className="mt-1 text-xs text-muted-foreground">
                  Download Minecraft versions and Java
                </p>
              </div>
            </div>

          </div>

          <div className="flex-1 min-h-0 flex flex-col">
            <div className="flex gap-1 shrink-0 rounded-lg bg-muted p-1 w-fit">
              <button
                className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  tab === "minecraft" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setTab("minecraft")}
              >
                Minecraft
              </button>
              <button
                className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  tab === "java" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setTab("java")}
              >
                Java
              </button>
              <button
                className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  tab === "chinese" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setTab("chinese")}
              >
                mcmod上搜索
              </button>
              <button
                className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
                  tab === "english" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                }`}
                onClick={() => setTab("english")}
              >
                全部平台搜索
              </button>
            </div>

            {/* Minecraft version download */}
            {tab === "minecraft" && <div className="flex-1 min-h-0 flex flex-col gap-4 mt-4">
              <div className="shrink-0">
                <VersionFilterBar
                  filter={versionFilter}
                  onFilterChange={setVersionFilter}
                  searchQuery={searchQuery}
                  onSearchChange={setSearchQuery}
                />
              </div>

              <AnimatePresence mode="wait">
                {loading ? (
                  <motion.div
                    key="loading"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Loader2 className="size-8 animate-spin" />
                    <p className="text-sm">Loading version list...</p>
                  </motion.div>
                ) : error ? (
                  <motion.div
                    key="error"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <AlertCircle className="size-8 text-destructive" />
                    <p className="text-sm">Failed to get version list</p>
                    <p className="text-xs">{error}</p>
                    <Button variant="outline" size="sm" onClick={refetch} className="mt-2 gap-2">
                      <RefreshCw className="size-3.5" />
                      Retry
                    </Button>
                  </motion.div>
                ) : (
                  <motion.div
                    key="list"
                    className="flex-1 min-h-0 flex flex-col"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <VersionList
                      versions={filteredVersions}
                      onSelectVersion={setSelectedVersion}
                    />
                  </motion.div>
                )}
              </AnimatePresence>
            </div>}

            {/* Java download */}
            {tab === "java" && <div className="flex-1 min-h-0 flex flex-col gap-4 mt-4">
              <div className="shrink-0">
                <div className="flex items-center justify-between gap-4">
                  <div className="flex items-center gap-1">
                    <Button
                      variant={javaVersionFilter === "all" ? "default" : "ghost"}
                      size="sm"
                      onClick={() => setJavaVersionFilter("all")}
                      className="text-xs"
                    >
                      All
                    </Button>
                    <Button
                      variant={javaVersionFilter === "jre" ? "default" : "ghost"}
                      size="sm"
                      onClick={() => setJavaVersionFilter("jre")}
                      className="text-xs"
                    >
                      JRE
                    </Button>
                    <Button
                      variant={javaVersionFilter === "jdk" ? "default" : "ghost"}
                      size="sm"
                      onClick={() => setJavaVersionFilter("jdk")}
                      className="text-xs"
                    >
                      JDK
                    </Button>
                  </div>
                  <div className="relative w-64">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
                    <Input
                      placeholder="Search Java versions..."
                      value={javaSearchQuery}
                      onChange={(e) => setJavaSearchQuery(e.target.value)}
                      className="pl-9 h-8 text-sm"
                    />
                  </div>
                </div>
              </div>

              <AnimatePresence mode="wait">
                {javaVersionsLoading ? (
                  <motion.div
                    key="loading"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Loader2 className="size-8 animate-spin" />
                    <p className="text-sm">Loading version list...</p>
                  </motion.div>
                ) : javaMessage && javaMessage.type === "error" ? (
                  <motion.div
                    key="error"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <AlertCircle className="size-8 text-destructive" />
                    <p className="text-sm">Failed to get version list</p>
                    <p className="text-xs">{javaMessage.text}</p>
                    <Button variant="outline" size="sm" onClick={loadJavaVersions} className="mt-2 gap-2">
                      <RefreshCw className="size-3.5" />
                      Retry
                    </Button>
                  </motion.div>
                ) : filteredJavaVersions.length === 0 ? (
                  <motion.div
                    key="empty"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground py-16"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Coffee className="size-10 opacity-40" />
                    <p className="text-sm">No matching Java versions found</p>
                  </motion.div>
                ) : (
                  <motion.div
                    key="list"
                    className="flex-1 min-h-0 overflow-y-auto rounded-xl border border-border bg-card"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    {filteredJavaVersions.map((v, index) => {
                      const isDownloading = tasks.some(t =>
                        t.label === v.name && t.status === "downloading"
                      );
                      const task = tasks.find(t => t.label === v.name);
                      return (
                        <button
                          key={v.name}
                          type="button"
                          onClick={() => handleJavaDownload(v.name)}
                          className="group flex w-full items-center justify-between px-4 py-3 hover:bg-accent/50 transition-colors duration-200 border-b border-border last:border-b-0 text-left"
                          disabled={isDownloading}
                        >
                          <div className="flex items-center gap-4 min-w-0">
                            <div className="flex flex-col gap-0.5">
                              <span className="font-semibold text-sm leading-none">
                                {v.name}
                              </span>
                              <span className="text-xs text-muted-foreground">
                                {v.version}
                              </span>
                            </div>
                          </div>

                          {task && task.status === "downloading" && task.progress !== undefined && (
                            <div className="flex items-center gap-2">
                              <span className="text-xs text-muted-foreground">
                                {task.progress.toFixed(1)}%
                              </span>
                            </div>
                          )}

                          <div className="flex items-center gap-2">
                            {task && task.status === "downloading" ? (
                              <Loader2 className="size-4 text-primary animate-spin" />
                            ) : (
                              <Download className="size-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity duration-200" />
                            )}
                          </div>
                        </button>
                      );
                    })}
                  </motion.div>
                )}
              </AnimatePresence>
            </div>}

            {/* Chinese search (new version: based on local moddata.db) */}
            {tab === "chinese" && <div className="flex-1 min-h-0 flex flex-col gap-4 mt-4">
              <div className="shrink-0">
                <div className="flex items-center justify-between gap-4">
                  <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
                    <Input
                      placeholder="Enter Chinese keywords to search mods..."
                      value={chineseSearchQuery}
                      onChange={(e) => {
                        setChineseSearchQuery(e.target.value);
                        setChineseSearchError(null);
                      }}
                      onKeyDown={(e) => e.key === "Enter" && handleChineseSearch()}
                      className={`pl-9 h-8 text-sm ${
                        chineseSearchError ? "border-destructive focus-visible:ring-destructive" : ""
                      }`}
                    />
                  </div>
                  <Button
                    size="sm"
                    onClick={handleChineseSearch}
                    disabled={chineseSearchLoading || !chineseSearchQuery.trim()}
                  >
                    {chineseSearchLoading ? <Loader2 className="size-4 animate-spin" /> : "Search"}
                  </Button>
                </div>
                {/* Error message */}
                {chineseSearchError && (
                  <div className="flex items-center gap-2 mt-2 text-xs text-destructive">
                    <AlertCircle className="size-3.5" />
                    <span>{chineseSearchError}</span>
                  </div>
                )}
                {/* Hint */}
                {!chineseSearchError && !chineseSearchLoading && !chineseSearchResults && (
                  <p className="mt-2 text-xs text-muted-foreground">
                    💡 Enter Chinese keywords (e.g., "工业", "林业", "存储") to search in local offline database
                  </p>
                )}
              </div>

              <AnimatePresence mode="wait">
                {chineseSearchLoading ? (
                  <motion.div
                    key="loading"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Loader2 className="size-8 animate-spin" />
                    <p className="text-sm">Searching...</p>
                  </motion.div>
                ) : chineseSearchResults && chineseSearchResults.length > 0 ? (
                  <motion.div
                    key="results"
                    className="flex-1 min-h-0 overflow-y-auto rounded-xl border border-border bg-card"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <div className="divide-y divide-border">
                      {chineseSearchResults.map((result, index) => {
                        const mcmodUrl = result.mcmod_id ? `https://www.mcmod.cn/class/${result.mcmod_id}.html` : null;
                        return (
                          <button
                            key={index}
                            onClick={() => handleResultClick(result)}
                            className="group w-full text-left px-4 py-3 hover:bg-accent/50 transition-colors flex items-center gap-3"
                          >
                            <div className="flex flex-col gap-0.5 min-w-0 flex-1">
                              {/* Chinese name (main title) */}
                              <span className="font-semibold text-sm leading-tight truncate">
                                {result.chinese_name || result.slug}
                              </span>
                              {/* slug (subtitle, secondary info) */}
                              <span className="text-xs text-muted-foreground truncate">
                                {result.chinese_name ? result.slug : ""}
                              </span>
                            </div>
                            <div className="flex items-center gap-2 shrink-0">
                              {mcmodUrl && (
                                <a
                                  href={mcmodUrl}
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  onClick={(e) => e.stopPropagation()}
                                  className="text-xs text-muted-foreground hover:text-foreground inline-flex items-center gap-1 px-2 py-1 rounded border border-border hover:border-primary/50 transition-colors"
                                >
                                  <ExternalLink className="size-3" />
                                  MCMod
                                </a>
                              )}
                              <ArrowRight className="size-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
                            </div>
                          </button>
                        );
                      })}
                    </div>
                    {/* Result count hint */}
                    <div className="px-4 py-2 text-xs text-muted-foreground border-t border-border bg-muted/30">
                      Total {chineseSearchResults.length} results
                    </div>
                  </motion.div>
                ) : (
                  <motion.div
                    key="empty"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Search className="size-10 opacity-40" />
                    <p className="text-sm">
                      {chineseSearchResults ? "No matching mods found" : "Enter keywords to start searching"}
                    </p>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>}

            {/* English search */}
            {tab === "english" && <div className="flex-1 min-h-0 flex flex-col gap-4 mt-4">
              <div className="shrink-0 flex flex-col gap-3">
                <div className="flex items-center gap-3">
                  {/* Search bar */}
                  <div className="relative flex-1">
                    <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
                    <Input
                      placeholder="Search mods, modpacks, resource packs, shaders..."
                      value={englishQuery}
                      onChange={(e) => {
                        setEnglishQuery(e.target.value);
                        setEnglishError(null);
                      }}
                      onKeyDown={(e) => e.key === "Enter" && handleEnglishSearch()}
                      className={`pl-9 h-9 text-sm ${englishError ? "border-destructive focus-visible:ring-destructive" : ""}`}
                    />
                  </div>
                  <Button
                    size="sm"
                    onClick={handleEnglishSearch}
                    disabled={englishLoading || !englishQuery.trim()}
                    className="h-9"
                  >
                    {englishLoading ? <Loader2 className="size-4 animate-spin" /> : <Search className="size-4" />}
                    <span className="ml-1">Search</span>
                  </Button>
                </div>
                {/* Six category selection buttons */}
                <div className="flex flex-wrap items-center gap-2">
                  {ENGLISH_CATEGORIES.map((cat) => {
                    const Icon = cat.icon;
                    const isActive = englishCategory === cat.id;
                    return (
                      <button
                        key={cat.id}
                        onClick={() => setEnglishCategory(cat.id)}
                        className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-medium transition-all border ${
                          isActive
                            ? "bg-primary text-primary-foreground border-primary shadow-sm"
                            : "bg-background text-foreground hover:bg-accent border-border"
                        }`}
                      >
                        <Icon className="size-3.5" />
                        <span>{cat.short}</span>
                      </button>
                    );
                  })}
                </div>
                {englishError && (
                  <div className="flex items-center gap-2 text-xs text-destructive">
                    <AlertCircle className="size-3.5" />
                    <span>{englishError}</span>
                  </div>
                )}
                {!englishError && !englishLoading && !englishResults && (
                  <p className="text-xs text-muted-foreground">
                    Search both Modrinth and CurseForge, click results to view versions and download.
                  </p>
                )}
                {englishSourceInfo && (
                  <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                    <span className={`inline-flex items-center gap-1 ${englishSourceInfo.modrinth.ok ? "" : "text-destructive"}`}>
                      <span className="size-1.5 rounded-full bg-emerald-500" />
                      Modrinth {englishSourceInfo.modrinth.ok ? `(${englishSourceInfo.modrinth.count})` : "(unreachable)"}
                    </span>
                    <span className={`inline-flex items-center gap-1 ${englishSourceInfo.curseforge.ok ? "" : "text-destructive"}`}>
                      <span className="size-1.5 rounded-full bg-orange-500" />
                      CurseForge {englishSourceInfo.curseforge.ok ? `(${englishSourceInfo.curseforge.count})` : "(unreachable)"}
                    </span>
                  </div>
                )}
              </div>

              <AnimatePresence mode="wait">
                {englishLoading ? (
                  <motion.div
                    key="loading"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Loader2 className="size-8 animate-spin" />
                    <p className="text-sm">Searching Modrinth and CurseForge…</p>
                  </motion.div>
                ) : englishResults && englishResults.length > 0 ? (
                  <motion.div
                    key="results"
                    className="flex-1 min-h-0 overflow-y-auto rounded-xl border border-border bg-card"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <div className="divide-y divide-border">
                      {englishResults.map((result) => {
                        const path = categoryPath(
                          categoryForSource(result.projectType || englishCategory),
                          result.slug,
                          englishQuery,
                          englishCategory,
                        );
                        const Icon =
                          ENGLISH_CATEGORIES.find((c) => c.id === (categoryForSource(result.projectType || englishCategory) as EnglishCategory))
                            ?.icon ?? Box;
                        return (
                          <button
                            key={`${result.source}-${result.slug}`}
                            onClick={() => router.push(path)}
                            className="group w-full text-left px-4 py-3 hover:bg-accent/50 transition-colors flex items-start gap-3"
                          >
                            {/* Icon */}
                            <div className="shrink-0 w-12 h-12 rounded-lg bg-muted flex items-center justify-center overflow-hidden border border-border">
                              {result.iconUrl ? (
                                <img
                                  src={result.iconUrl}
                                  alt=""
                                  className="w-full h-full object-contain"
                                  onError={(e) => {
                                    (e.currentTarget as HTMLImageElement).style.display = "none";
                                  }}
                                />
                              ) : (
                                <Icon className="size-5 text-muted-foreground" />
                              )}
                            </div>

                            {/* Main info */}
                            <div className="flex-1 min-w-0 flex flex-col gap-1">
                              <div className="flex items-center gap-2 flex-wrap">
                                <span className="font-semibold text-sm leading-tight truncate">
                                  {result.title}
                                </span>
                                <Badge variant="outline" className="text-[10px] h-4 font-mono">
                                  {result.slug}
                                </Badge>
                                <Badge
                                  variant="secondary"
                                  className={`text-[10px] h-4 ${
                                    result.source === "both"
                                      ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 border-emerald-500/30"
                                      : result.source === "modrinth"
                                      ? "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 border-emerald-500/30"
                                      : "bg-orange-500/10 text-orange-700 dark:text-orange-300 border-orange-500/30"
                                  }`}
                                >
                                  {result.source === "both"
                                    ? "Modrinth + CurseForge"
                                    : result.source === "modrinth"
                                    ? "Modrinth"
                                    : "CurseForge"}
                                </Badge>
                              </div>
                              {result.description && (
                                <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                                  {result.description}
                                </p>
                              )}
                              <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground pt-0.5">
                                {result.author && (
                                  <span className="inline-flex items-center gap-1">
                                    <span className="text-foreground/70">Author</span>
                                    <span className="text-foreground">{result.author}</span>
                                  </span>
                                )}
                                {result.downloads !== undefined && (
                                  <span>· {formatNumber(result.downloads)} downloads</span>
                                )}
                                {result.updated && <span>· Updated {formatDate(result.updated)}</span>}
                                {result.mcVersions && result.mcVersions.length > 0 && (
                                  <span className="inline-flex items-center gap-1">
                                    · MC {result.mcVersions.slice(0, 3).join(", ")}
                                    {result.mcVersions.length > 3 ? `, +${result.mcVersions.length - 3}` : ""}
                                  </span>
                                )}
                              </div>
                              {result.categories && result.categories.length > 0 && (
                                <div className="flex flex-wrap items-center gap-1 pt-1">
                                  {result.categories.slice(0, 5).map((c) => (
                                    <Badge key={c} variant="secondary" className="text-[10px] h-4">
                                      {c}
                                    </Badge>
                                  ))}
                                </div>
                              )}
                            </div>

                            <div className="shrink-0 flex flex-col items-end gap-1">
                              <ArrowRight className="size-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
                              {result.externalUrl && (
                                <a
                                  href={result.externalUrl}
                                  target="_blank"
                                  rel="noopener noreferrer"
                                  onClick={(e) => e.stopPropagation()}
                                  className="text-[10px] text-muted-foreground hover:text-foreground inline-flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity"
                                >
                                  <ExternalLink className="size-3" />
                                  Open Source
                                </a>
                              )}
                            </div>
                          </button>
                        );
                      })}
                    </div>
                    <div className="px-4 py-2 text-xs text-muted-foreground border-t border-border bg-muted/30">
                      {englishResults.length} results
                    </div>
                  </motion.div>
                ) : englishResults ? (
                  <motion.div
                    key="empty"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Coffee className="size-10 opacity-40" />
                    <p className="text-sm">No matching projects found</p>
                  </motion.div>
                ) : (
                  <motion.div
                    key="idle"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Search className="size-10 opacity-40" />
                    <p className="text-sm">Enter keywords to start searching</p>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>}
          </div>
        </motion.div>
      )}
    </AnimatePresence>

    </>
  );
}