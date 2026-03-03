"use client";

import { useState, useMemo } from "react";
import { Download, Loader2, AlertCircle, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  VersionFilterBar,
  type VersionFilter,
} from "@/components/download/version-filter-bar";
import { VersionList } from "@/components/download/version-list";
import { VersionDetail } from "@/components/download/version-detail";
import { useMinecraftVersions } from "@/hooks/use-minecraft-versions";
import type { MinecraftVersion } from "@/types";

/**
 * 下载页面
 * 版本列表视图 → 点击版本进入详情选择加载器 → 选择加载器版本
 */
export default function DownloadPage() {
  const { versions, loading, error, refetch } = useMinecraftVersions();
  const [versionFilter, setVersionFilter] = useState<VersionFilter>("release");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedVersion, setSelectedVersion] =
    useState<MinecraftVersion | null>(null);

  const filteredVersions = useMemo(() => {
    return versions.filter((v) => {
      if (versionFilter !== "all" && v.type !== versionFilter) return false;
      if (
        searchQuery &&
        !v.id.toLowerCase().includes(searchQuery.toLowerCase())
      )
        return false;
      return true;
    });
  }, [versions, versionFilter, searchQuery]);

  // 版本详情视图
  if (selectedVersion) {
    return (
      <div className="flex h-full flex-col p-4">
        <VersionDetail
          version={selectedVersion}
          onBack={() => setSelectedVersion(null)}
        />
      </div>
    );
  }

  // 版本列表视图
  return (
    <div className="flex h-full flex-col gap-4 p-4">
      {/* 页面标题 */}
      <div className="flex items-center gap-3 shrink-0">
        <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
          <Download className="size-5 text-primary" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none">版本下载</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            选择版本，开始你的 Minecraft 之旅
          </p>
        </div>
      </div>

      {/* 筛选栏 */}
      <div className="shrink-0">
        <VersionFilterBar
          filter={versionFilter}
          onFilterChange={setVersionFilter}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
        />
      </div>

      {/* 加载状态 */}
      {loading && (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
          <Loader2 className="size-8 animate-spin" />
          <p className="text-sm">正在获取版本列表...</p>
        </div>
      )}

      {/* 错误状态 */}
      {error && !loading && (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
          <AlertCircle className="size-8 text-destructive" />
          <p className="text-sm">获取版本列表失败</p>
          <p className="text-xs">{error}</p>
          <Button variant="outline" size="sm" onClick={refetch} className="mt-2 gap-2">
            <RefreshCw className="size-3.5" />
            重试
          </Button>
        </div>
      )}

      {/* 版本列表 */}
      {!loading && !error && (
        <VersionList
          versions={filteredVersions}
          onSelectVersion={setSelectedVersion}
        />
      )}
    </div>
  );
}
