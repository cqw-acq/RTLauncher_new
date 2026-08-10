"use client";

import { useCallback, useEffect, useState, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  PackageOpen,
  ChevronRight,
  Plus,
  CheckCircle2,
  Circle,
  Loader2,
  ChevronDown,
  Search,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { InstanceData } from "@/types";
import { useLaunchContext } from "./launch-provider";
import { useI18n } from "@/components/i18n/use-i18n";
import { LoaderIcon, inferLoaderKind } from "@/components/launch/loader-icon";

interface McVersionGroup {
  mcVersion: string;
  instances: InstanceData[];
}

/** 比较两个 MC 版本号，返回正表示 a > b，负表示 a < b（降序用） */
function compareMcVersionDesc(a: string, b: string): number {
  // 检测周快照格式（如 25w42a）
  const weeklySnapshotRegex = /^(\d{2})w(\d{2})([a-z])$/;
  const aMatch = a.match(weeklySnapshotRegex);
  const bMatch = b.match(weeklySnapshotRegex);

  // 如果都是周快照，按（年、周、字母）排序
  if (aMatch && bMatch) {
    const aYear = parseInt(aMatch[1], 10);
    const aWeek = parseInt(aMatch[2], 10);
    const aLetter = aMatch[3];
    const bYear = parseInt(bMatch[1], 10);
    const bWeek = parseInt(bMatch[2], 10);
    const bLetter = bMatch[3];

    if (aYear !== bYear) return bYear - aYear; // 降序
    if (aWeek !== bWeek) return bWeek - aWeek; // 降序
    return bLetter.localeCompare(aLetter); // 字母降序
  }

  // 如果只有一个是周快照，周快照排在后面
  if (aMatch) return -1;
  if (bMatch) return 1;

  // 对于点分版本号（如 1.20.1），逐段比较
  const parseVersionParts = (s: string): number[] => {
    const parts = s.split(".").map(part => {
      const num = parseInt(part, 10);
      return isNaN(num) ? 0 : num;
    });
    return parts;
  };

  const aParts = parseVersionParts(a);
  const bParts = parseVersionParts(b);

  // 逐位比较
  const maxLength = Math.max(aParts.length, bParts.length);
  for (let i = 0; i < maxLength; i++) {
    const aVal = aParts[i] || 0;
    const bVal = bParts[i] || 0;
    if (aVal !== bVal) {
      return bVal - aVal; // 降序
    }
  }

  // 如果数字部分相同，按字典序比较作为回退
  return b.localeCompare(a);
}

interface VersionSidebarProps {
  className?: string;
}

/**
 * 版本管理侧边栏
 * 显示已安装的游戏版本列表，支持切换版本
 */
export function VersionSidebar({ className }: VersionSidebarProps) {
  const { t } = useI18n();
  const { config, updateConfig } = useLaunchContext();
  const [instances, setInstances] = useState<InstanceData[]>([]);
  const [loading, setLoading] = useState(true);
  const [isExpanded, setIsExpanded] = useState(true);
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [searchQuery, setSearchQuery] = useState("");

  // 按 MC 版本分组（或搜索过滤）
  const groupedInstances = useMemo<McVersionGroup[]>(() => {
    const q = searchQuery.toLowerCase();

    if (q) {
      // 搜索模式：返回所有匹配的实例，按原始MC版本分组
      const matchingInstances = instances.filter(instance =>
        instance.name.toLowerCase().includes(q) ||
        instance.minecraft_version.toLowerCase().includes(q)
      );

      const groups = new Map<string, InstanceData[]>();
      for (const instance of matchingInstances) {
        const mcVersion = instance.minecraft_version || "Unknown";
        if (!groups.has(mcVersion)) {
          groups.set(mcVersion, []);
        }
        groups.get(mcVersion)!.push(instance);
      }

      return Array.from(groups.entries())
        .map(([mcVersion, instances]) => ({ mcVersion, instances }))
        .sort((a, b) => compareMcVersionDesc(a.mcVersion, b.mcVersion));
    } else {
      // 正常模式：按MC版本分组
      const groups = new Map<string, InstanceData[]>();

      for (const instance of instances) {
        const mcVersion = instance.minecraft_version || "Unknown";
        if (!groups.has(mcVersion)) {
          groups.set(mcVersion, []);
        }
        groups.get(mcVersion)!.push(instance);
      }

      return Array.from(groups.entries())
        .map(([mcVersion, instances]) => ({ mcVersion, instances }))
        .sort((a, b) => compareMcVersionDesc(a.mcVersion, b.mcVersion));
    }
  }, [instances, searchQuery]);

  // 默认展开所有分组（搜索时展开所有，正常时也展开所有）
  useEffect(() => {
    if (groupedInstances.length > 0) {
      setExpandedGroups(new Set(groupedInstances.map(g => g.mcVersion)));
    }
  }, [groupedInstances]);

  // 加载已安装的游戏版本。版本安装在 .minecraft/versions 下，而不是
  // 一个独立的 instances 目录；路径不一致会导致侧边栏始终显示为空。
  const loadInstances = useCallback(async () => {
    if (!config.minecraftPath) {
      setInstances([]);
      setLoading(false);
      return;
    }

    try {
      setLoading(true);
      const instancesPath = `${config.minecraftPath}/versions`;
      const result = await invoke<InstanceData[]>("vm_scan_instances", {
        instancesPath,
      });
      setInstances(result || []);
    } catch (error) {
      console.error("Failed to load instances:", error);
      setInstances([]);
    } finally {
      setLoading(false);
    }
  }, [config.minecraftPath]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadInstances();
    }, 0);

    return () => window.clearTimeout(timer);
  }, [loadInstances]);

  const handleSelectVersion = (instance: InstanceData) => {
    // 更新启动配置中的版本信息
    updateConfig({
      versionName: instance.name,
      // 基础 Minecraft 版本（用于定位游戏 JAR）
      minecraftVersion: instance.minecraft_version,
      loadType: instance.loader === "Vanilla" ? "0" : "1",
      loadName: instance.loader === "Vanilla" ? "" : instance.name,
    });
  };

  const toggleGroup = (mcVersion: string) => {
    setExpandedGroups(prev => {
      const next = new Set(prev);
      if (next.has(mcVersion)) {
        next.delete(mcVersion);
      } else {
        next.add(mcVersion);
      }
      return next;
    });
  };

  return (
    <div
      className={cn(
        "flex flex-col border-r border-border bg-sidebar transition-all duration-300",
        isExpanded ? "w-64" : "w-12",
        className
      )}
    >
      {/* 头部 */}
      <div className="flex flex-col border-b border-border shrink-0">
        <div className="flex items-center justify-between p-3 h-14">
          <AnimatePresence mode="wait">
            {isExpanded && (
              <motion.div
                initial={{ opacity: 0, x: -10 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -10 }}
                transition={{ duration: 0.2 }}
                className="flex items-center gap-2"
              >
                <PackageOpen className="size-4 text-primary" />
                <span className="text-sm font-medium">{t("launch.versionSidebar.gameVersions")}</span>
              </motion.div>
            )}
          </AnimatePresence>

          <Button
            variant="ghost"
            size="icon"
            className="size-7 shrink-0"
            onClick={() => setIsExpanded(!isExpanded)}
          >
            <ChevronRight
              className={cn(
                "size-4 transition-transform duration-300",
                isExpanded && "rotate-180"
              )}
            />
          </Button>
        </div>

        {/* 搜索框 */}
        <AnimatePresence>
          {isExpanded && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              transition={{ duration: 0.2 }}
              className="px-3 pb-2"
            >
              <div className="relative">
                <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
                <Input
                  placeholder={t("launch.versionSelector.searchVersions")}
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="h-7 pl-8 pr-7 text-xs"
                />
                {searchQuery && (
                  <button
                    onClick={() => setSearchQuery("")}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  >
                    <X className="size-3" />
                  </button>
                )}
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* 版本列表 */}
      <div className="flex-1 overflow-y-auto">
        <div className="p-2 space-y-1">
          {loading ? (
            <div className="flex items-center justify-center py-8">
              <Loader2 className="size-5 animate-spin text-muted-foreground" />
            </div>
          ) : instances.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 px-3 text-center">
              <PackageOpen className="size-8 text-muted-foreground/40 mb-2" />
              {isExpanded && (
                <>
                  <p className="text-xs text-muted-foreground mb-1">
                    {t("launch.versionSidebar.noInstalledVersions")}
                  </p>
                  <p className="text-[10px] text-muted-foreground/60">
                    {t("launch.versionSidebar.installAGameVersionFromDownloads")}
                  </p>
                </>
              )}
            </div>
          ) : (
            groupedInstances.map((group) => {
              const isGroupExpanded = expandedGroups.has(group.mcVersion);

              return (
                <div key={group.mcVersion}>
                  {/* MC 版本分组标题 */}
                  <motion.button
                    onClick={() => toggleGroup(group.mcVersion)}
                    className={cn(
                      "w-full flex items-center gap-2 rounded-lg p-2 transition-colors text-left",
                      "hover:bg-accent/50 font-medium text-xs"
                    )}
                    whileHover={{ scale: 1.01 }}
                    whileTap={{ scale: 0.99 }}
                  >
                    <ChevronDown
                      className={cn(
                        "size-3.5 transition-transform duration-200",
                        !isGroupExpanded && "rotate-[-90deg]"
                      )}
                    />
                    <span>{group.mcVersion}</span>
                    <span className="text-[10px] text-muted-foreground opacity-60">
                      ({group.instances.length})
                    </span>
                  </motion.button>

                  {/* 该 MC 版本下的实例列表 */}
                  <AnimatePresence>
                    {isGroupExpanded && (
                      <motion.div
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: "auto" }}
                        exit={{ opacity: 0, height: 0 }}
                        transition={{ duration: 0.2 }}
                        className="space-y-1 ml-2"
                      >
                        {group.instances.map((instance) => {
                          const isSelected =
                            config.loadName === instance.name || config.versionName === instance.name;

                          return (
                            <motion.button
                              key={instance.name}
                              onClick={() => handleSelectVersion(instance)}
                              className={cn(
                                "w-full flex items-center gap-2 rounded-lg p-2 transition-colors text-left",
                                "hover:bg-accent/50",
                                isSelected && "bg-accent text-accent-foreground"
                              )}
                              whileHover={{ scale: 1.02 }}
                              whileTap={{ scale: 0.98 }}
                            >
                              {/* 选中指示器 + Loader 图标 */}
                              <div className="flex shrink-0 items-center gap-1.5">
                                {isExpanded ? (
                                  isSelected ? (
                                    <CheckCircle2 className="size-4 shrink-0 text-primary" />
                                  ) : (
                                    <Circle className="size-4 shrink-0 opacity-40" />
                                  )
                                ) : (
                                  <div
                                    className={cn(
                                      "size-2 rounded-full shrink-0",
                                      isSelected ? "bg-primary" : "bg-muted-foreground/40"
                                    )}
                                  />
                                )}
                                <LoaderIcon
                                  kind={inferLoaderKind(instance.loader)}
                                  className="size-6"
                                />
                              </div>

                              {/* 版本信息 */}
                              <AnimatePresence mode="wait">
                                {isExpanded && (
                                  <motion.div
                                    initial={{ opacity: 0, x: -10 }}
                                    animate={{ opacity: 1, x: 0 }}
                                    exit={{ opacity: 0, x: -10 }}
                                    transition={{ duration: 0.2 }}
                                    className="flex-1 min-w-0"
                                  >
                                    <div className="font-medium text-xs truncate">
                                      {instance.name}
                                    </div>
                                    <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
                                      <span>{instance.loader}</span>
                                      {instance.mods_count > 0 && (
                                        <>
                                          <span className="opacity-40">·</span>
                                          <span>{instance.mods_count} Mods</span>
                                        </>
                                      )}
                                    </div>
                                  </motion.div>
                                )}
                              </AnimatePresence>
                            </motion.button>
                          );
                        })}
                      </motion.div>
                    )}
                  </AnimatePresence>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* 底部操作按钮 */}
      {isExpanded && (
        <div className="border-t border-border p-2">
          <Button
            variant="outline"
            size="sm"
            className="w-full text-xs gap-2"
            onClick={loadInstances}
          >
            <Plus className="size-3" />
            {t("launch.versionSidebar.refreshList")}
          </Button>
        </div>
      )}
    </div>
  );
}