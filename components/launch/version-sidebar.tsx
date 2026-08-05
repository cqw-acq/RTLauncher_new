"use client";

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  PackageOpen,
  ChevronRight,
  Plus,
  CheckCircle2,
  Circle,
  Loader2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { InstanceData } from "@/types";
import { useLaunchContext } from "./launch-provider";
import { useI18n } from "@/components/i18n/use-i18n";
import { LoaderIcon, inferLoaderKind } from "@/components/launch/loader-icon";

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
      loadType: instance.loader === "Vanilla" ? "0" : "1",
      loadName: instance.loader === "Vanilla" ? "" : instance.name,
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
      <div className="flex items-center justify-between border-b border-border p-3 h-14 shrink-0">
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
            instances.map((instance) => {
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
                          {instance.minecraft_version}
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