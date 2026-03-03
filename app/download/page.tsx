"use client";

import { useState, useMemo } from "react";
import { Download } from "lucide-react";
import {
  VersionFilterBar,
  type VersionFilter,
} from "@/components/download/version-filter-bar";
import { VersionList } from "@/components/download/version-list";
import { VersionDetail } from "@/components/download/version-detail";
import { MINECRAFT_VERSIONS } from "@/constants/data";
import type { MinecraftVersion } from "@/types";

/**
 * 下载页面
 * 版本列表视图 → 点击版本进入详情选择加载器
 */
export default function DownloadPage() {
  const [versionFilter, setVersionFilter] = useState<VersionFilter>("release");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedVersion, setSelectedVersion] =
    useState<MinecraftVersion | null>(null);

  const filteredVersions = useMemo(() => {
    return MINECRAFT_VERSIONS.filter((v) => {
      if (versionFilter !== "all" && v.type !== versionFilter) return false;
      if (
        searchQuery &&
        !v.id.toLowerCase().includes(searchQuery.toLowerCase())
      )
        return false;
      return true;
    });
  }, [versionFilter, searchQuery]);

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

      {/* 版本列表 */}
      <VersionList
        versions={filteredVersions}
        onSelectVersion={setSelectedVersion}
      />
    </div>
  );
}
