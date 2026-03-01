"use client";

import { useState, useMemo } from "react";
import { Download } from "lucide-react";
import { LoaderSelector } from "@/components/download/loader-selector";
import {
  VersionFilterBar,
  type VersionFilter,
} from "@/components/download/version-filter-bar";
import { VersionList } from "@/components/download/version-list";
import { MINECRAFT_VERSIONS } from "@/constants/data";
import type { LoaderType } from "@/types";

/**
 * 下载页面
 * 选择加载器类型，筛选并浏览 Minecraft 版本
 */
export default function DownloadPage() {
  const [selectedLoader, setSelectedLoader] = useState<LoaderType>("vanilla");
  const [versionFilter, setVersionFilter] = useState<VersionFilter>("all");
  const [searchQuery, setSearchQuery] = useState("");

  const filteredVersions = useMemo(() => {
    return MINECRAFT_VERSIONS.filter((v) => {
      // 版本类型筛选
      if (versionFilter !== "all" && v.type !== versionFilter) return false;
      // 搜索过滤
      if (searchQuery && !v.id.toLowerCase().includes(searchQuery.toLowerCase()))
        return false;
      return true;
    });
  }, [versionFilter, searchQuery]);

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
            选择加载器和版本，开始你的 Minecraft 之旅
          </p>
        </div>
      </div>

      {/* 加载器选择 */}
      <div className="shrink-0">
        <LoaderSelector
          selectedLoader={selectedLoader}
          onSelect={setSelectedLoader}
        />
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
      <VersionList versions={filteredVersions} />
    </div>
  );
}
