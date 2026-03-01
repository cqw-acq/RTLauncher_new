"use client";

import { VersionListItem } from "@/components/download/version-list-item";
import { PackageOpen } from "lucide-react";
import type { MinecraftVersion } from "@/types";

interface VersionListProps {
  versions: MinecraftVersion[];
}

export function VersionList({ versions }: VersionListProps) {
  if (versions.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground py-16">
        <PackageOpen className="size-10 opacity-40" />
        <p className="text-sm">没有找到匹配的版本</p>
      </div>
    );
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto rounded-xl border border-border bg-card">
      {versions.map((version) => (
        <VersionListItem key={version.id} version={version} />
      ))}
    </div>
  );
}
