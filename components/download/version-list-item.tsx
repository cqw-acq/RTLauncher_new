"use client";

import { Badge } from "@/components/ui/badge";
import { ChevronRight } from "lucide-react";
import type { MinecraftVersion } from "@/types";

interface VersionListItemProps {
  version: MinecraftVersion;
  onSelect: (version: MinecraftVersion) => void;
}

export function VersionListItem({ version, onSelect }: VersionListItemProps) {
  return (
    <button
      type="button"
      onClick={() => onSelect(version)}
      className="group flex w-full items-center justify-between px-4 py-3 hover:bg-accent/50 transition-colors duration-200 border-b border-border last:border-b-0 text-left"
    >
      {/* 左侧：版本号 + 发布日期 */}
      <div className="flex items-center gap-4 min-w-0">
        <div className="flex flex-col gap-0.5">
          <span className="font-semibold text-sm leading-none">
            {version.id}
          </span>
          <span className="text-xs text-muted-foreground">
            {version.releaseDate}
          </span>
        </div>
      </div>

      {/* 中间：版本类型标签 */}
      <div className="flex items-center gap-2">
        {version.isLatest && (
          <Badge variant="default" className="text-[10px] px-1.5 py-0">
            最新
          </Badge>
        )}
        <Badge
          variant={version.type === "release" ? "secondary" : "outline"}
          className="text-[10px] px-1.5 py-0"
        >
          {version.type === "release" ? "正式版" : "快照"}
        </Badge>
      </div>

      {/* 右侧：箭头指示 */}
      <ChevronRight className="size-4 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity duration-200" />
    </button>
  );
}
