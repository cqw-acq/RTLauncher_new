"use client";

import { useState, useEffect } from "react";
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
  Circle,
  ChevronDown,
} from "lucide-react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { useLaunchContext } from "./launch-provider";
import type { DirEntry } from "@/types";

interface VersionInfo {
  name: string;
  type: string;
}

/**
 * 版本选择对话框
 * 从 versions 目录扫描已安装的游戏版本
 */
export function VersionSelectorDialog() {
  const { config, updateConfig } = useLaunchContext();
  const [open, setOpen] = useState(false);
  const [versions, setVersions] = useState<VersionInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  // 打开对话框时加载版本列表
  useEffect(() => {
    if (open) {
      loadVersions();
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

      // 使用 vm_list_dir 列出 versions 目录
      const entries = await invoke<DirEntry[]>("vm_list_dir", {
        dirPath: versionsPath,
        extensionsFilter: [],
      });

      // 只保留目录，并推断类型
      const versionList: VersionInfo[] = entries
        .filter((entry) => entry.is_dir)
        .map((entry) => {
          const name = entry.name.toLowerCase();
          let type = "Vanilla";

          if (name.includes("fabric")) {
            type = "Fabric";
          } else if (name.includes("neoforge")) {
            type = "NeoForge";
          } else if (name.includes("forge")) {
            type = "Forge";
          } else if (name.includes("quilt")) {
            type = "Quilt";
          }

          return {
            name: entry.name,
            type,
          };
        });

      setVersions(versionList);
    } catch (error) {
      console.error("Failed to load versions:", error);
      setVersions([]);
    } finally {
      setLoading(false);
    }
  };

  const handleSelectVersion = (version: VersionInfo) => {
    updateConfig({
      versionName: version.name,
      loadType: version.type === "Vanilla" ? "0" : "1",
      loadName: version.type === "Vanilla" ? "" : version.name,
    });
    setOpen(false);
  };

  // 过滤版本列表
  const filteredVersions = versions.filter(
    (v) =>
      v.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      v.type.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          variant="outline"
          className="w-full justify-between text-xs h-8"
        >
          <span className="truncate">
            {config.versionName || "选择游戏版本"}
          </span>
          <ChevronDown className="size-3 ml-2 shrink-0" />
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-md max-h-[80vh] overflow-hidden">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <PackageOpen className="size-4 text-primary" />
            选择游戏版本
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-4 overflow-hidden">
          {/* 搜索框 */}
          <div className="relative">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
            <Input
              placeholder="搜索版本..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="pl-9 text-xs"
            />
          </div>

          {/* 版本列表 */}
          <div className="max-h-96 overflow-y-auto overflow-x-hidden space-y-1">
            {loading ? (
              <div className="flex items-center justify-center py-12">
                <Loader2 className="size-6 animate-spin text-muted-foreground" />
              </div>
            ) : filteredVersions.length === 0 ? (
              <div className="flex flex-col items-center justify-center py-12 text-center">
                <PackageOpen className="size-10 text-muted-foreground/40 mb-3" />
                <p className="text-sm text-muted-foreground mb-1">
                  {searchQuery ? "没有找到匹配的版本" : "暂无已安装版本"}
                </p>
                <p className="text-xs text-muted-foreground/60">
                  前往下载页面安装游戏
                </p>
              </div>
            ) : (
              filteredVersions.map((version) => {
                const isSelected = config.versionName === version.name;

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
                    {/* 选中指示器 */}
                    {isSelected ? (
                      <CheckCircle2 className="size-4 shrink-0 text-primary" />
                    ) : (
                      <Circle className="size-4 shrink-0 opacity-40" />
                    )}

                    {/* 版本信息 */}
                    <div className="flex-1 min-w-0 overflow-hidden">
                      <div className="font-medium text-sm truncate">
                        {version.name}
                      </div>
                      <div className="flex items-center gap-2 text-xs text-muted-foreground mt-0.5">
                        <span
                          className={cn(
                            "px-1.5 py-0.5 rounded text-[10px] font-medium",
                            version.type === "Vanilla" &&
                              "bg-green-500/10 text-green-600 dark:text-green-400",
                            version.type === "Forge" &&
                              "bg-orange-500/10 text-orange-600 dark:text-orange-400",
                            version.type === "Fabric" &&
                              "bg-purple-500/10 text-purple-600 dark:text-purple-400",
                            version.type === "NeoForge" &&
                              "bg-blue-500/10 text-blue-600 dark:text-blue-400",
                            version.type === "Quilt" &&
                              "bg-pink-500/10 text-pink-600 dark:text-pink-400"
                          )}
                        >
                          {version.type}
                        </span>
                      </div>
                    </div>
                  </motion.button>
                );
              })
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
              刷新列表
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

