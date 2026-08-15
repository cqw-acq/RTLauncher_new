"use client";

import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Search } from "lucide-react";
import { cn } from "@/lib/utils";
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n";
import type { MinecraftVersionType } from "@/types";

type VersionFilter = MinecraftVersionType;

interface VersionFilterBarProps {
  filter: VersionFilter;
  onFilterChange: (filter: VersionFilter) => void;
  searchQuery: string;
  onSearchChange: (query: string) => void;
}

const filterOptions: { labelKey: TranslationKey; value: VersionFilter }[] = [
  { labelKey: "download.versionType.release", value: "release" },
  { labelKey: "download.versionType.snapshot", value: "snapshot" },
  { labelKey: "download.versionType.aprilFools", value: "april_fools" },
  { labelKey: "download.versionType.oldVersion", value: "old_version" },
];

export function VersionFilterBar({
  filter,
  onFilterChange,
  searchQuery,
  onSearchChange,
}: VersionFilterBarProps) {
  const { t } = useI18n();
  return (
    <div className="flex items-center justify-between gap-4">
      {/* 版本类型筛选 */}
      <div className="flex items-center gap-1">
        {filterOptions.map((option) => (
          <Button
            key={option.value}
            variant="ghost"
            size="sm"
            onClick={() => onFilterChange(option.value)}
            className={cn(
              "text-xs transition-colors duration-200",
              filter === option.value
                ? "bg-accent text-accent-foreground font-medium"
                : "text-muted-foreground hover:text-foreground"
            )}
          >
            {t(option.labelKey)}
          </Button>
        ))}
      </div>

      {/* 搜索框 */}
      <div className="relative w-64">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
        <Input
          placeholder={t("download.searchVersionPlaceholder")}
          value={searchQuery}
          onChange={(e) => onSearchChange(e.target.value)}
          className="pl-9 h-8 text-sm"
        />
      </div>
    </div>
  );
}

export type { VersionFilter };