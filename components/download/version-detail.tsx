"use client";

import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { CombinedInstall } from "@/components/download/combined-install";
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n";
import { ArrowLeft } from "lucide-react";
import type { MinecraftVersion } from "@/types";

const versionTypeLabelKeys: Record<string, TranslationKey> = {
  release: "download.versionType.release",
  snapshot: "download.versionType.snapshot",
  april_fools: "download.versionType.aprilFools",
  old_version: "download.versionType.oldVersion",
};

interface VersionDetailProps {
  version: MinecraftVersion;
  onBack: () => void;
}

export function VersionDetail({ version, onBack }: VersionDetailProps) {
  const { t } = useI18n();

  return (
    <div className="flex h-full flex-col gap-4">
      {/* 返回按钮 + 版本信息头 */}
      <div className="flex items-center gap-3 shrink-0">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onBack}
          aria-label={t("download.backToVersionList")}
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
                {t("download.latest")}
              </Badge>
            )}
            <Badge
              variant={version.type === "release" ? "secondary" : "outline"}
              className="text-[10px] px-1.5 py-0"
            >
              {versionTypeLabelKeys[version.type]
                ? t(versionTypeLabelKeys[version.type])
                : version.type}
            </Badge>
          </div>
        </div>
        <span className="text-xs text-muted-foreground ml-auto">
          {t("download.released", { date: version.releaseDate })}
        </span>
      </div>

      <CombinedInstall version={version} />
    </div>
  );
}