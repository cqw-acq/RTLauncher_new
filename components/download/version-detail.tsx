"use client";

import { useState, useEffect, useCallback } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { LoaderSelector } from "@/components/download/loader-selector";
import { LoaderVersionList } from "@/components/download/loader-version-list";
import { useDownloadManager } from "@/components/download/download-provider";
import { ArrowLeft } from "lucide-react";
import { slideInFromRight, slideInFromLeft } from "@/lib/motion";
import { invoke } from "@tauri-apps/api/core";
import type { MinecraftVersion, LoaderType, LoaderVersion, LauncherPathsConfig } from "@/types";
import { LOADER_OPTIONS } from "@/constants/data";

const versionTypeLabels: Record<string, string> = {
  release: "正式版",
  snapshot: "快照",
  april_fools: "愚人节",
  old_version: "远古版",
};

/** 从后端获取加载器版本列表 */
async function fetchLoaderVersions(
  mcVersion: string,
  loader: LoaderType
): Promise<LoaderVersion[]> {
  switch (loader) {
    case "fabric": {
      const versions = await invoke<string[]>("get_fabric_versions", {
        mcVersion,
        useMirror: false,
      });
      return versions.map((v) => ({
        id: `fabric-${v}`,
        version: v,
        releaseDate: "",
      }));
    }
    case "forge": {
      const versions = await invoke<string[]>("get_forge_versions", {
        mcVersion,
      });
      return versions.map((v) => ({
        id: `forge-${v}`,
        version: v,
        releaseDate: "",
      }));
    }
    case "quilt": {
      const versions = await invoke<string[]>("get_quilt_versions", {
        mcVersion,
      });
      return versions.map((v) => ({
        id: `quilt-${v}`,
        version: v,
        releaseDate: "",
      }));
    }
    case "neoforge": {
      const versions = await invoke<string[]>("get_neoforge_versions", {
        mcVersion,
      });
      return versions.map((v) => ({
        id: `neoforge-${v}`,
        version: v,
        releaseDate: "",
      }));
    }
    case "optifine": {
      const versions = await invoke<string[]>("list_optifine_versions", {
        mcVersion,
      });
      return versions.map((v) => ({
        id: `optifine-${v}`,
        version: v,
        releaseDate: "",
      }));
    }
    case "liteloader": {
      const versions = await invoke<string[]>("get_liteloader_versions", {
        mcVersion,
      });
      return versions.map((v) => ({
        id: `liteloader-${v}`,
        version: v,
        releaseDate: "",
      }));
    }
    default:
      return [];
  }
}

/** 调用后端安装加载器 */
async function installLoader(
  loader: LoaderType,
  mcVersion: string,
  loaderVersion: string,
  mcPath: string
) {
  switch (loader) {
    case "fabric":
      await invoke("install_fabric", {
        mcVersion,
        loaderVersion,
        mcFolderPath: mcPath,
      });
      break;
    case "forge":
      await invoke("install_forge", {
        mcVersion,
        forgeVersion: loaderVersion,
        downloadDir: mcPath,
      });
      break;
    case "quilt":
      await invoke("install_quilt", {
        mcVersion,
        quiltVersion: loaderVersion,
        mcFolderPath: mcPath,
      });
      break;
    case "neoforge":
      await invoke("install_neoforge", {
        mcVersion,
        neoforgeVersion: loaderVersion,
        downloadDir: mcPath,
      });
      break;
    case "optifine":
      // OptiFine standalone install requires a wrapper jar path
      // For now, place in mods folder (with-forge mode)
      await invoke("install_optifine_with_forge", {
        optifineVersion: loaderVersion,
        minecraftDir: mcPath,
      });
      break;
    case "liteloader":
      await invoke("install_liteloader", {
        mcVersion,
        liteVersion: loaderVersion,
        minecraftDir: mcPath,
      });
      break;
  }
}

interface VersionDetailProps {
  version: MinecraftVersion;
  onBack: () => void;
}

export function VersionDetail({ version, onBack }: VersionDetailProps) {
  const [selectedLoader, setSelectedLoader] = useState<LoaderType | null>(null);
  const { startDownload } = useDownloadManager();

  // 加载器版本列表
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>([]);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);

  // 安装状态
  const [installing, setInstalling] = useState(false);
  const [installStatus, setInstallStatus] = useState<string>("");
  const [installError, setInstallError] = useState<string | null>(null);

  // 当选中非 vanilla 加载器时，从后端获取版本列表
  useEffect(() => {
    if (!selectedLoader || selectedLoader === "vanilla") {
      setLoaderVersions([]);
      setFetchError(null);
      return;
    }

    let cancelled = false;
    setLoadingVersions(true);
    setLoaderVersions([]);
    setFetchError(null);

    fetchLoaderVersions(version.id, selectedLoader)
      .then((versions) => {
        if (!cancelled) setLoaderVersions(versions);
      })
      .catch((e) => {
        if (!cancelled) setFetchError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoadingVersions(false);
      });

    return () => {
      cancelled = true;
    };
  }, [selectedLoader, version.id]);

  const handleSelectLoader = useCallback(
    (loaderId: LoaderType) => {
      if (loaderId === "vanilla") {
        startDownload(`Minecraft ${version.id}`, version.id);
        return;
      }
      setSelectedLoader(loaderId);
      setInstallError(null);
      setInstallStatus("");
    },
    [startDownload, version.id]
  );

  const handleInstallLoaderVersion = useCallback(
    async (loaderVersion: LoaderVersion) => {
      if (!selectedLoader) return;

      const loaderName =
        LOADER_OPTIONS.find((l) => l.id === selectedLoader)?.name ??
        selectedLoader;

      setInstalling(true);
      setInstallError(null);
      setInstallStatus(`正在下载基础版本并安装 ${loaderName}...`);

      try {
        // 同时启动：下载原版 MC + 安装加载器
        startDownload(`Minecraft ${version.id}`, version.id);

        // 获取 .minecraft 路径
        const config = await invoke<LauncherPathsConfig>(
          "get_launcher_paths_config"
        );
        const mcPath = config.selected_minecraft_path;

        // 安装加载器（下载加载器的库文件，与原版下载并行）
        setInstallStatus(`正在安装 ${loaderName} ${loaderVersion.version}...`);
        await installLoader(
          selectedLoader,
          version.id,
          loaderVersion.version,
          mcPath
        );

        setInstallStatus("安装完成！");
      } catch (e) {
        setInstallError(String(e));
        setInstallStatus("");
      } finally {
        setInstalling(false);
      }
    },
    [selectedLoader, startDownload, version.id]
  );

  const selectedLoaderInfo = selectedLoader
    ? LOADER_OPTIONS.find((l) => l.id === selectedLoader)
    : null;

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
              setInstallError(null);
              setInstallStatus("");
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

      {/* 安装状态提示 */}
      {(installStatus || installError) && (
        <div className="shrink-0">
          {installError ? (
            <div className="rounded-lg border border-destructive/50 bg-destructive/10 px-4 py-2 text-sm text-destructive">
              {installError}
            </div>
          ) : (
            <div className="rounded-lg border border-primary/30 bg-primary/5 px-4 py-2 text-sm text-primary flex items-center gap-2">
              {installing && (
                <div className="size-3.5 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
              )}
              {installStatus}
            </div>
          )}
        </div>
      )}

      {/* 内容区域 */}
      <div className="flex-1 min-h-0 overflow-y-auto p-1">
        <AnimatePresence mode="wait">
          {selectedLoader && selectedLoaderInfo ? (
            <motion.div
              key="loader-versions"
              variants={slideInFromRight}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <LoaderVersionList
                loaderName={selectedLoaderInfo.name}
                versions={loaderVersions}
                loading={loadingVersions}
                error={fetchError}
                installing={installing}
                onInstall={handleInstallLoaderVersion}
              />
            </motion.div>
          ) : (
            <motion.div
              key="loader-selector"
              variants={slideInFromLeft}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <LoaderSelector
                versionId={version.id}
                onSelectLoader={handleSelectLoader}
              />
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </div>
  );
}
