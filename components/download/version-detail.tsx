"use client";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { LoaderSelector } from "@/components/download/loader-selector";
import { ArrowLeft } from "lucide-react";
import type { MinecraftVersion } from "@/types";

interface VersionDetailProps {
  version: MinecraftVersion;
  onBack: () => void;
}

export function VersionDetail({ version, onBack }: VersionDetailProps) {
  return (
    <div className="flex h-full flex-col gap-4">
      {/* 返回按钮 + 版本信息头 */}
      <div className="flex items-center gap-3 shrink-0">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onBack}
          aria-label="返回版本列表"
        >
          <ArrowLeft className="size-4" />
        </Button>
        <div className="flex items-center gap-3">
          <h2 className="text-lg font-semibold leading-none">
            Minecraft {version.id}
          </h2>
          <div className="flex items-center gap-1.5">
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
        </div>
        <span className="text-xs text-muted-foreground ml-auto">
          发布于 {version.releaseDate}
        </span>
      </div>

      {/* 选择加载器标题 */}
      <div className="shrink-0">
        <h3 className="text-sm font-medium text-muted-foreground">
          选择加载器
        </h3>
        <p className="text-xs text-muted-foreground/70 mt-0.5">
          选择一个加载器来安装此版本
        </p>
      </div>

      {/* 加载器卡片网格 */}
      <div className="flex-1 min-h-0 overflow-y-auto p-1">
        <LoaderSelector versionId={version.id} />
      </div>
    </div>
  );
}
