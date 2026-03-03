"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { LoaderSelector } from "@/components/download/loader-selector";
import { LoaderVersionList } from "@/components/download/loader-version-list";
import { useDownloadManager } from "@/components/download/download-provider";
import { ArrowLeft } from "lucide-react";
import type { MinecraftVersion, LoaderType, LoaderVersion } from "@/types";
import { LOADER_OPTIONS, LOADER_VERSIONS } from "@/constants/data";

const versionTypeLabels: Record<string, string> = {
  release: "正式版",
  snapshot: "快照",
  april_fools: "愚人节",
  old_version: "远古版",
};

interface VersionDetailProps {
  version: MinecraftVersion;
  onBack: () => void;
}

export function VersionDetail({ version, onBack }: VersionDetailProps) {
  const [selectedLoader, setSelectedLoader] = useState<LoaderType | null>(null);
  const { startDownload } = useDownloadManager();

  const handleSelectLoader = (loaderId: LoaderType) => {
    if (loaderId === "vanilla") {
      startDownload(`Minecraft ${version.id}`, version.id);
      return;
    }
    setSelectedLoader(loaderId);
  };

  const handleInstallLoaderVersion = (loaderVersion: LoaderVersion) => {
    const loaderName =
      LOADER_OPTIONS.find((l) => l.id === selectedLoader)?.name ??
      selectedLoader;
    startDownload(
      `${version.id} + ${loaderName} ${loaderVersion.version}`,
      version.id
    );
  };

  const selectedLoaderInfo = selectedLoader
    ? LOADER_OPTIONS.find((l) => l.id === selectedLoader)
    : null;

  const loaderVersions = selectedLoader
    ? LOADER_VERSIONS[selectedLoader] ?? []
    : [];

  return (
    <div className="flex h-full flex-col gap-4">
      {/* 返回按钮 + 版本信息头 */}
      <div className="flex items-center gap-3 shrink-0">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => {
            if (selectedLoader) {
              setSelectedLoader(null);
            } else {
              onBack();
            }
          }}
          aria-label={selectedLoader ? "返回加载器选择" : "返回版本列表"}
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
              {versionTypeLabels[version.type] ?? version.type}
            </Badge>
          </div>
        </div>
        <span className="text-xs text-muted-foreground ml-auto">
          发布于 {version.releaseDate}
        </span>
      </div>

      {/* 子标题区域 */}
      <div className="shrink-0">
        {selectedLoader && selectedLoaderInfo ? (
          <>
            <h3 className="text-sm font-medium text-muted-foreground">
              选择 {selectedLoaderInfo.name} 版本
            </h3>
            <p className="text-xs text-muted-foreground/70 mt-0.5">
              选择一个 {selectedLoaderInfo.name} 版本进行安装
            </p>
          </>
        ) : (
          <>
            <h3 className="text-sm font-medium text-muted-foreground">
              选择加载器
            </h3>
            <p className="text-xs text-muted-foreground/70 mt-0.5">
              选择一个加载器来安装此版本
            </p>
          </>
        )}
      </div>

      {/* 内容区域 */}
      <div className="flex-1 min-h-0 overflow-y-auto p-1">
        {selectedLoader && selectedLoaderInfo ? (
          <LoaderVersionList
            loaderName={selectedLoaderInfo.name}
            versions={loaderVersions}
            onInstall={handleInstallLoaderVersion}
          />
        ) : (
          <LoaderSelector
            versionId={version.id}
            onSelectLoader={handleSelectLoader}
          />
        )}
      </div>
    </div>
  );
}
