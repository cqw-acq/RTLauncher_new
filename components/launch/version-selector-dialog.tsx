"use client";

import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  PackageOpen,
  Search,
  Loader2,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
} from "lucide-react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n";
import { useLaunchContext } from "./launch-provider";
import { LoaderIcon, inferLoaderKind, type LoaderKind } from "@/components/launch/loader-icon";

/** 解析后的版本信息 */
interface ParsedVersion {
  name: string;           // 完整目录名
  mcVersion: string;      // MC 版本号 如 1.20.1
  loaderType: string;     // 加载器类型：vanilla/fabric/forge/neoforge/optifine/liteloader/quilt
  loaderVersion: string;  // 加载器版本号 如 0.15.11
}

/** 层级数据结构 */
interface McVersionNode {
  mcVersion: string;
  loaders: Record<string, ParsedVersion[]>;  // loaderType -> 版本列表
}

type Step = "mc" | "loader" | "version";

interface VersionSelectorDialogProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  compact?: boolean;
}

/** 从目录名解析版本信息 */
function parseVersionDir(dirName: string): ParsedVersion {
  const lower = dirName.toLowerCase();

  // 检测加载器类型
  let loaderType: string = "vanilla";
  let loaderVersion = "";
  let mcVersion = dirName;

  // 按优先级匹配（neoforge 要在 forge 之前检测，因为 neoforge 包含 forge）
  if (lower.includes("neoforge")) {
    loaderType = "neoforge";
  } else if (lower.includes("liteloader")) {
    loaderType = "liteloader";
  } else if (lower.includes("optifine")) {
    loaderType = "optifine";
  } else if (lower.includes("fabric")) {
    loaderType = "fabric";
  } else if (lower.includes("forge")) {
    loaderType = "forge";
  } else if (lower.includes("quilt")) {
    loaderType = "quilt";
  }

  if (loaderType !== "vanilla") {
    // 尝试从目录名中提取 MC 版本号和加载器版本号
    // 常见格式: 1.20.1-fabric-0.15.11, 1.20.1-forge-47.2.0, 1.20.1-neoforge-21.1.99
    const parts = dirName.split("-");

    // 第一个通常是 MC 版本号（如 1.20.1）
    if (/^\d+\.\d+/.test(parts[0])) {
      mcVersion = parts[0];

      // 从剩余部分提取加载器版本号（找到第一个看起来像版本号的部分）
      for (let i = 2; i < parts.length; i++) {
        if (/^\d+/.test(parts[i])) {
          // 可能是加载器版本号的起始，收集连续的版本片段
          const loaderParts: string[] = [];
          for (let j = i; j < parts.length; j++) {
            loaderParts.push(parts[j]);
          }
          loaderVersion = loaderParts.join("-");
          break;
        }
      }

      // 如果没提取到，就把加载器名后面的都算进去
      if (!loaderVersion && parts.length > 2) {
        loaderVersion = parts.slice(2).join("-");
      }
    }
  }

  return {
    name: dirName,
    mcVersion,
    loaderType,
    loaderVersion: loaderVersion || dirName,
  };
}

/** 加载器类型的显示信息 */
const LOADER_DISPLAY: Record<string, { label: TranslationKey; color: string; order: number }> = {
  vanilla:    { label: "launch.versionSelector.vanilla", color: "bg-green-500/10 text-green-600 dark:text-green-400", order: 0 },
  forge:      { label: "launch.versionSelector.forge", color: "bg-orange-500/10 text-orange-600 dark:text-orange-400", order: 1 },
  fabric:     { label: "launch.versionSelector.fabric", color: "bg-purple-500/10 text-purple-600 dark:text-purple-400", order: 2 },
  neoforge:   { label: "launch.versionSelector.neoForge", color: "bg-blue-500/10 text-blue-600 dark:text-blue-400", order: 3 },
  quilt:      { label: "launch.versionSelector.quilt", color: "bg-pink-500/10 text-pink-600 dark:text-pink-400", order: 4 },
  optifine:   { label: "launch.versionSelector.optiFine", color: "bg-cyan-500/10 text-cyan-600 dark:text-cyan-400", order: 5 },
  liteloader: { label: "launch.versionSelector.liteLoader", color: "bg-yellow-500/10 text-yellow-600 dark:text-yellow-400", order: 6 },
};

function parsedVersionFromInstance(instance: ScannedInstance): ParsedVersion {
  const fromName = parseVersionDir(instance.name);
  const detectedLoader = instance.loader.trim().toLowerCase();
  const loaderType = Object.prototype.hasOwnProperty.call(LOADER_DISPLAY, detectedLoader)
    ? detectedLoader
    : fromName.loaderType;

  return {
    name: instance.name,
    mcVersion: instance.minecraft_version || fromName.mcVersion,
    loaderType,
    loaderVersion:
      fromName.loaderType === loaderType && fromName.loaderVersion !== instance.name
        ? fromName.loaderVersion
        : instance.name,
  };
}

export function VersionSelectorDialog({ open: controlledOpen, onOpenChange, compact }: VersionSelectorDialogProps = {}) {
  const { t } = useI18n();
  const { config, updateConfig } = useLaunchContext();
  const [internalOpen, setInternalOpen] = useState(false);
  const [versions, setVersions] = useState<ParsedVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [mcSearchQuery, setMcSearchQuery] = useState("");
  const [loaderSearchQuery, setLoaderSearchQuery] = useState("");
  const [versionSearchQuery, setVersionSearchQuery] = useState("");

  // 层级选择的状态
  const [step, setStep] = useState<Step>("mc");
  const [selectedMcVersion, setSelectedMcVersion] = useState<string | null>(null);
  const [selectedLoader, setSelectedLoader] = useState<string | null>(null);

  const isControlled = controlledOpen !== undefined;
  const open = isControlled ? controlledOpen : internalOpen;
  const setOpen = (value: boolean) => {
    if (isControlled) {
      onOpenChange?.(value);
    } else {
      setInternalOpen(value);
    }
  };

  useEffect(() => {
    if (open) {
      loadVersions();
      setStep("mc");
      setSelectedMcVersion(null);
      setSelectedLoader(null);
      setMcSearchQuery("");
      setLoaderSearchQuery("");
      setVersionSearchQuery("");
    }
  }, [open, config.minecraftPath]);

  const loadVersions = async () => {
    if (!config.minecraftPath) {
      setVersions([]);
      return;
    }

    try {
      setLoading(true);
      const versionsPath = `${config.minecraftPath}/versions`;

      const instances = await invoke<ScannedInstance[]>("vm_scan_instances", {
        instancesPath: versionsPath,
      });

      const parsedList = instances.map(parsedVersionFromInstance);

      setVersions(parsedList);
    } catch (error) {
      console.error("Failed to load versions:", error);
      setVersions([]);
    } finally {
      setLoading(false);
    }
  };

  // 构建数据结构：按 mcVersion 组织，每个 mcVersion 下按 loaderType 组织版本
  const mcTree = useMemo<McVersionNode[]>(() => {
    const mcMap = new Map<string, Map<string, ParsedVersion[]>>();

    for (const v of versions) {
      if (!mcMap.has(v.mcVersion)) {
        mcMap.set(v.mcVersion, new Map());
      }
      const loaderMap = mcMap.get(v.mcVersion)!;
      if (!loaderMap.has(v.loaderType)) {
        loaderMap.set(v.loaderType, []);
      }
      loaderMap.get(v.loaderType)!.push(v);
    }

    // 转换为数组并排序
    return Array.from(mcMap.entries())
      .map(([mcVersion, loaderMap]) => {
        const loaders: Record<string, ParsedVersion[]> = {};
        for (const [loaderType, list] of loaderMap.entries()) {
          loaders[loaderType] = list;
        }
        return { mcVersion, loaders };
      })
      .sort((a, b) => compareMcVersionDesc(a.mcVersion, b.mcVersion));
  }, [versions]);

  // 当前选中 MC 节点
  const currentMcNode = useMemo(
    () => mcTree.find((n) => n.mcVersion === selectedMcVersion) || null,
    [mcTree, selectedMcVersion]
  );

  // 7 种固定的 loader 类型（包括原版 vanilla）
  const AVAILABLE_LOADERS = ["vanilla", "forge", "liteloader", "neoforge", "optifine", "fabric", "quilt"];

  // 第二步：当前 MC 版本下，7 种 loader 各有多少个版本
  const loaderAvailability = useMemo(() => {
    if (!currentMcNode) return [];
    return AVAILABLE_LOADERS.map((lt) => ({
      type: lt,
      versions: currentMcNode.loaders[lt] || [],
    }));
  }, [currentMcNode]);

  // 第三步：当前 MC 版本 + loader 下的版本列表
  // 需求2：modloader 选中后，version 列表要包含原版 + modloader 版本
  const currentVersionList = useMemo(() => {
    if (!currentMcNode || !selectedLoader) return [];
    const loaderVersions = currentMcNode.loaders[selectedLoader] || [];
    if (selectedLoader === "vanilla") {
      return loaderVersions;
    }
    // 非 vanilla 的 modloader：将原版版本和 modloader 版本合并
    const vanillaVersions = currentMcNode.loaders["vanilla"] || [];
    return [...vanillaVersions, ...loaderVersions];
  }, [currentMcNode, selectedLoader]);

  // 第一步：过滤 MC 版本（同时搜索子版本实例名）
  const filteredMcTree = useMemo(() => {
    const q = mcSearchQuery.toLowerCase();
    const wrapped = mcTree.map((n) => {
      const matchingInstances: ParsedVersion[] = [];
      if (q) {
        for (const loaderType of Object.keys(n.loaders)) {
          for (const inst of n.loaders[loaderType]) {
            if (inst.name.toLowerCase().includes(q)) {
              matchingInstances.push(inst);
            }
          }
        }
      }
      return { node: n, matchingInstances, matchesMc: q ? n.mcVersion.toLowerCase().includes(q) : true };
    });

    if (!q) return wrapped;

    return wrapped.filter((item) => item.matchesMc || item.matchingInstances.length > 0);
  }, [mcTree, mcSearchQuery]);

  // 第二步：过滤 loader
  const filteredLoaders = useMemo(() => {
    if (!loaderSearchQuery) return loaderAvailability;
    const q = loaderSearchQuery.toLowerCase();
    return loaderAvailability.filter((item) => {
      const display = LOADER_DISPLAY[item.type]?.label;
      return (
        (display ? t(display).toLowerCase().includes(q) : false) ||
        item.type.toLowerCase().includes(q)
      );
    });
  }, [loaderAvailability, loaderSearchQuery, t]);

  // 第三步：过滤版本列表
  const filteredVersionList = useMemo(() => {
    if (!versionSearchQuery) return currentVersionList;
    const q = versionSearchQuery.toLowerCase();
    return currentVersionList.filter((v) =>
      v.name.toLowerCase().includes(q) || v.loaderVersion.toLowerCase().includes(q)
    );
  }, [currentVersionList, versionSearchQuery]);

  const handleSelectMcVersion = (mcVersion: string) => {
    setSelectedMcVersion(mcVersion);
    setLoaderSearchQuery("");
    setStep("loader");
  };

  const handleSelectLoader = (loaderType: string) => {
    setSelectedLoader(loaderType);
    setVersionSearchQuery("");
    setStep("version");
  };

  const handleSelectVersion = (version: ParsedVersion) => {
    const isVanilla = version.loaderType === "vanilla";
    updateConfig({
      // 原版版本号（如 1.20.1）
      versionName: version.mcVersion,
      // 加载器类型：0=原版，1=modloader
      loadType: isVanilla ? "0" : "1",
      // modloader 文件夹名（如 1.20.1-forge-47.2.0），原版时为空
      loadName: isVanilla ? "" : version.name,
    });
    setOpen(false);
  };

  const goBack = () => {
    if (step === "version") {
      setStep("loader");
      setSelectedLoader(null);
      setVersionSearchQuery("");
    } else if (step === "loader") {
      setStep("mc");
      setSelectedMcVersion(null);
      setLoaderSearchQuery("");
    }
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {!isControlled && (
        <DialogTrigger asChild>
          <Button
            variant="outline"
            className={cn(
              "w-full justify-between text-xs",
              compact ? "h-7 px-2" : "h-8"
            )}
          >
            <span className="truncate">
              {config.versionName || t("launch.versionSelector.selectAGameVersion")}
            </span>
            <ChevronDown className="size-3 ml-2 shrink-0" />
          </Button>
        </DialogTrigger>
      )}
      <DialogContent className="max-w-md max-h-[80vh] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <PackageOpen className="size-4 text-primary" />
            {t("launch.versionSelector.selectAGameVersion")}
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 overflow-hidden">
          {/* 步骤标题 / 返回按钮 */}
          {step !== "mc" && (
            <div className="flex items-center gap-2 -mt-1">
              <Button
                variant="ghost"
                size="sm"
                onClick={goBack}
                className="h-7 px-2 text-xs"
              >
                <ChevronLeft className="size-3.5 mr-0.5" />
                {t("common.back")}
              </Button>
              <div className="text-xs text-muted-foreground">
                {step === "loader" && (
                  <>
                    MC {t("launch.versionSelector.version")}:{" "}
                    <span className="font-medium text-foreground">
                      {selectedMcVersion}
                    </span>
                  </>
                )}
                {step === "version" && (
                  <>
                    {selectedMcVersion} ·{" "}
                    <span className="font-medium text-foreground">
                      {LOADER_DISPLAY[selectedLoader || ""] ? t(LOADER_DISPLAY[selectedLoader || ""].label) : selectedLoader}
                    </span>
                  </>
                )}
              </div>
            </div>
          )}

          {/* 搜索框（每步都显示） */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
            <Input
              placeholder={
                step === "mc"
                  ? t("launch.versionSelector.searchVanillaVersions")
                  : step === "loader"
                  ? t("launch.versionSelector.searchModLoaders")
                  : t("launch.versionSelector.searchVersions")
              }
              value={
                step === "mc"
                  ? mcSearchQuery
                  : step === "loader"
                  ? loaderSearchQuery
                  : versionSearchQuery
              }
              onChange={(e) => {
                if (step === "mc") setMcSearchQuery(e.target.value);
                else if (step === "loader") setLoaderSearchQuery(e.target.value);
                else setVersionSearchQuery(e.target.value);
              }}
              className="pl-9 text-xs"
            />
          </div>

          {/* 主内容区 */}
          <div className="max-h-96 overflow-y-auto overflow-x-hidden space-y-1">
            {loading ? (
              <div className="flex items-center justify-center py-12">
                <Loader2 className="size-6 animate-spin text-muted-foreground" />
              </div>
            ) : step === "mc" ? (
              filteredMcTree.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                  <PackageOpen className="size-10 text-muted-foreground/40 mb-3" />
                  <p className="text-sm text-muted-foreground mb-1">
                    {mcSearchQuery ? t("launch.versionSelector.noMatchingVersionsFound") : t("launch.versionSidebar.noInstalledVersions")}
                  </p>
                  <p className="text-xs text-muted-foreground/60">
                    {t("launch.versionSidebar.installAGameVersionFromDownloads")}
                  </p>
                </div>
              ) : (
                filteredMcTree.map((item) => {
                  const node = item.node;
                  const totalCount = Object.keys(node.loaders).reduce(
                    (sum, k) => sum + (node.loaders[k]?.length || 0),
                    0
                  );

                  return (
                    <motion.button
                      key={node.mcVersion}
                      onClick={() => handleSelectMcVersion(node.mcVersion)}
                      className={cn(
                        "w-full flex items-center gap-3 rounded-lg p-3 transition-colors text-left",
                        "hover:bg-accent/50"
                      )}
                      whileHover={{ scale: 1.01 }}
                      whileTap={{ scale: 0.99 }}
                    >
                      <LoaderIcon kind="vanilla" className="size-6 shrink-0" />

                      <div className="flex-1 min-w-0 overflow-hidden">
                        <div className="font-medium text-sm truncate">
                          {node.mcVersion}
                        </div>
                        {mcSearchQuery && item.matchingInstances.length > 0 ? (
                          <div className="text-[11px] text-muted-foreground/70 mt-1 flex flex-wrap gap-1">
                            {item.matchingInstances.slice(0, 5).map((inst) => (
                              <span
                                key={inst.name}
                                className="inline-block px-1.5 py-0.5 rounded bg-accent/60 text-foreground/80 truncate max-w-[120px]"
                              >
                                {inst.name}
                              </span>
                            ))}
                            {item.matchingInstances.length > 5 && (
                              <span className="text-muted-foreground/50">
                                +{item.matchingInstances.length - 5}
                              </span>
                            )}
                          </div>
                        ) : (
                          <div className="text-[11px] text-muted-foreground/70 mt-1">
                            {t("launch.versionSelector.totalCountSubversions", { totalCount: totalCount })}
                          </div>
                        )}
                      </div>

                      <ChevronDown className="size-4 shrink-0 text-muted-foreground rotate-[-90deg]" />
                    </motion.button>
                  );
                })
              )
            ) : step === "loader" ? (
              currentMcNode ? (
                filteredLoaders.length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-12 text-center">
                    <PackageOpen className="size-10 text-muted-foreground/40 mb-3" />
                    <p className="text-sm text-muted-foreground">
                      {loaderSearchQuery ? t("launch.versionSelector.noMatchingModLoadersFound") : t("launch.versionSelector.noModLoadersAvailable")}
                    </p>
                  </div>
                ) : (
                  filteredLoaders.map((item) => {
                  const display = LOADER_DISPLAY[item.type];
                  const count = item.versions.length;
                  const preview =
                    count > 0
                      ? item.versions
                          .slice(0, 2)
                          .map((v) => v.loaderVersion)
                          .join(", ") + (count > 2 ? "..." : "")
                      : "";

                  return (
                    <motion.button
                      key={item.type}
                      onClick={() => handleSelectLoader(item.type)}
                      className={cn(
                        "w-full flex items-center gap-3 rounded-lg p-3 transition-colors text-left",
                        "hover:bg-accent/50",
                        count === 0 && "opacity-50"
                      )}
                      whileHover={count > 0 ? { scale: 1.01 } : {}}
                      whileTap={count > 0 ? { scale: 0.99 } : {}}
                    >
                      <LoaderIcon kind={inferLoaderKind(item.type) as LoaderKind} className="size-7 shrink-0" />

                      <div className="flex-1 min-w-0 overflow-hidden">
                        <div className="flex items-center gap-2">
                          <span
                            className={cn(
                              "px-1.5 py-0.5 rounded text-[10px] font-medium",
                              display?.color ||
                                "bg-gray-500/10 text-gray-600 dark:text-gray-400"
                            )}
                          >
                            {display ? t(display.label) : item.type}
                          </span>
                          <span className="text-xs text-muted-foreground">
                            {t("launch.versionSelector.countVersions", { count: count })}
                          </span>
                        </div>
                        {preview && (
                          <div className="text-[11px] text-muted-foreground/70 mt-1 truncate">
                            {preview}
                          </div>
                        )}
                      </div>

                      <ChevronDown className="size-4 shrink-0 text-muted-foreground rotate-[-90deg]" />
                    </motion.button>
                  );
                  })
                )
              ) : (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                  <PackageOpen className="size-10 text-muted-foreground/40 mb-3" />
                  <p className="text-sm text-muted-foreground">
                    {t("launch.versionSelector.noLoadersAreAvailableForThisMcVersion")}
                  </p>
                </div>
              )
            ) : (
              // 第三步：版本列表
              filteredVersionList.length > 0 ? (
                filteredVersionList.map((version) => {
                  const isSelected =
                    config.versionName === version.mcVersion &&
                    (version.loaderType === "vanilla"
                      ? config.loadType === "0"
                      : config.loadName === version.name);
                  const display = LOADER_DISPLAY[version.loaderType];

                  return (
                    <motion.button
                      key={version.name}
                      onClick={() => handleSelectVersion(version)}
                      className={cn(
                        "w-full flex items-center gap-3 rounded-lg p-3 transition-colors text-left",
                        "hover:bg-accent/50",
                        isSelected && "bg-accent text-accent-foreground"
                      )}
                      whileHover={{ scale: 1.01 }}
                      whileTap={{ scale: 0.99 }}
                    >
                      {isSelected ? (
                        <div className="shrink-0 flex items-center">
                          <CheckCircle2 className="size-4 shrink-0 text-primary" />
                        </div>
                      ) : (
                        <LoaderIcon
                          kind={inferLoaderKind(version.loaderType) as LoaderKind}
                          className="size-6 shrink-0"
                        />
                      )}

                      <div className="flex-1 min-w-0 overflow-hidden">
                        <div className="font-medium text-sm truncate">
                          {version.name}
                        </div>
                        <div className="flex items-center gap-2 text-xs text-muted-foreground mt-0.5">
                          {version.loaderType !== "vanilla" && (
                            <span className="text-[11px] text-muted-foreground/80">
                              {t("launch.versionSelector.loaderVersion")}: {version.loaderVersion}
                            </span>
                          )}
                          <span
                            className={cn(
                              "px-1.5 py-0.5 rounded text-[10px] font-medium",
                              display?.color ||
                                "bg-gray-500/10 text-gray-600 dark:text-gray-400"
                            )}
                          >
                            {display ? t(display.label) : version.loaderType}
                          </span>
                        </div>
                      </div>
                    </motion.button>
                  );
                })
              ) : (
                <div className="flex flex-col items-center justify-center py-12 text-center">
                  <PackageOpen className="size-10 text-muted-foreground/40 mb-3" />
                  <p className="text-sm text-muted-foreground">
                    {versionSearchQuery ? t("launch.versionSelector.noMatchingVersionsFound") : t("launch.versionSelector.noVersionsAvailable")}
                  </p>
                </div>
              )
            )}
          </div>

          {/* 刷新按钮 */}
          <div className="flex justify-end gap-2 pt-2 border-t">
            <Button
              variant="outline"
              size="sm"
              onClick={loadVersions}
              disabled={loading}
            >
              {loading ? (
                <Loader2 className="size-3 animate-spin mr-1" />
              ) : (
                <PackageOpen className="size-3 mr-1" />
              )}
              {t("launch.versionSidebar.refreshList")}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** 比较两个 MC 版本号，返回正表示 a > b，负表示 a < b（降序用） */
function compareMcVersionDesc(a: string, b: string): number {
  const parseNum = (s: string) => {
    const parts = s.split(".").map((p) => {
      const n = parseInt(p, 10);
      return isNaN(n) ? 0 : n;
    });
    while (parts.length < 3) parts.push(0);
    return parts.slice(0, 3);
  };
  const pa = parseNum(a);
  const pb = parseNum(b);
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pb[i] - pa[i];
  }
  return 0;
}
interface ScannedInstance {
  name: string;
  minecraft_version: string;
  loader: string;
  mods_count: number;
}