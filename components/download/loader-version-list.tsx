"use client";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Download, Star } from "lucide-react";
import type { LoaderVersion } from "@/types";

interface LoaderVersionListProps {
  loaderName: string;
  versions: LoaderVersion[];
  onInstall: (version: LoaderVersion) => void;
}

export function LoaderVersionList({
  loaderName,
  versions,
  onInstall,
}: LoaderVersionListProps) {
  if (versions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 py-12 text-muted-foreground">
        <p className="text-sm">暂无可用的 {loaderName} 版本</p>
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-border bg-card overflow-y-auto">
      {versions.map((version) => (
        <div
          key={version.id}
          className="group flex items-center justify-between px-4 py-3 hover:bg-accent/50 transition-colors duration-200 border-b border-border last:border-b-0"
        >
          {/* 左侧：版本号 + 日期 */}
          <div className="flex items-center gap-3 min-w-0">
            <div className="flex flex-col gap-0.5">
              <span className="font-semibold text-sm leading-none">
                {version.version}
              </span>
              <span className="text-xs text-muted-foreground">
                {version.releaseDate}
              </span>
            </div>
          </div>

          {/* 中间：标签 */}
          <div className="flex items-center gap-2">
            {version.isRecommended && (
              <Badge
                variant="default"
                className="text-[10px] px-1.5 py-0 gap-1"
              >
                <Star className="size-2.5" />
                推荐
              </Badge>
            )}
          </div>

          {/* 右侧：安装按钮 */}
          <Button
            variant="ghost"
            size="icon-sm"
            className="opacity-0 group-hover:opacity-100 transition-opacity duration-200"
            onClick={() => onInstall(version)}
            aria-label={`安装 ${version.version}`}
          >
            <Download className="size-4" />
          </Button>
        </div>
      ))}
    </div>
  );
}
