"use client";

import { useState, useEffect, useRef } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { LoaderSelector } from "@/components/download/loader-selector";
import { LoaderVersionList } from "@/components/download/loader-version-list";
import { FabricApiDetail } from "@/components/download/fabric-api-detail";
import { QuiltApiDetail } from "@/components/download/quilt-api-detail";
import { useDownloadManager } from "@/components/download/download-provider";
import { ArrowLeft, Loader2 } from "lucide-react";
import { slideInFromRight, slideInFromLeft, fadeIn } from "@/lib/motion";
import type { MinecraftVersion, LoaderType, LoaderVersion } from "@/types";
import { LOADER_OPTIONS, LOADER_VERSIONS } from "@/constants/data";
import { invoke } from "@tauri-apps/api/core";
import { useLaunchContext } from "@/components/launch/launch-provider";

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

interface PendingDownloadInfo {
  kind: "vanilla" | "loader";
  loaderType?: LoaderType;
  loaderVersion?: LoaderVersion;
  defaultName: string;
}

export function VersionDetail({ version, onBack }: VersionDetailProps) {
  const [selectedLoader, setSelectedLoader] = useState<LoaderType | null>(null);
  const [showFabricApi, setShowFabricApi] = useState(false);
  const [showQuiltApi, setShowQuiltApi] = useState(false);
  const [optifineVersions, setOptifineVersions] = useState<LoaderVersion[]>([]);
  const [loadingOptifine, setLoadingOptifine] = useState(false);
  const [fabricLoaderVersions, setFabricLoaderVersions] = useState<LoaderVersion[]>([]);
  const [fabricApiVersions, setFabricApiVersions] = useState<LoaderVersion[]>([]);
  const [loadingFabric, setLoadingFabric] = useState(false);
  const [quiltLoaderVersions, setQuiltLoaderVersions] = useState<LoaderVersion[]>([]);
  const [loadingQuilt, setLoadingQuilt] = useState(false);
  const [forgeVersions, setForgeVersions] = useState<LoaderVersion[]>([]);
  const [loadingForge, setLoadingForge] = useState(false);
  const [neoforgeVersions, setNeoforgeVersions] = useState<LoaderVersion[]>([]);
  const [loadingNeoforge, setLoadingNeoforge] = useState(false);
  const [liteloaderVersions, setLiteloaderVersions] = useState<LoaderVersion[]>([]);
  const [loadingLiteloader, setLoadingLiteloader] = useState(false);
  const [showNameDialog, setShowNameDialog] = useState(false);
  const [pendingDownload, setPendingDownload] = useState<PendingDownloadInfo | null>(null);
  const [instanceNameInput, setInstanceNameInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const { startDownload, startOptifineDownload, startFabricDownload, startQuiltDownload, startForgeDownload, startNeoForgeDownload, startLiteLoaderDownload } = useDownloadManager();
  const { config } = useLaunchContext();

  // 当选择Optifine加载器时，获取Optifine版本列表
  useEffect(() => {
    if (selectedLoader === "optifine") {
      const fetchOptifineVersions = async () => {
        setLoadingOptifine(true);
        try {
          const result = await invoke<{ id: string; filename: string; self_version: string; full_version: string; download_url: string; official_url: string; is_pre: boolean }[]>(
            "get_optifine_versions",
            { mcVersion: version.id }
          );
          const versions: LoaderVersion[] = result.map(v => ({
            id: v.download_url,
            version: v.self_version,
            filename: v.download_url,
            official_url: v.official_url,
            releaseDate: "",
            isRecommended: !v.is_pre
          }));
          setOptifineVersions(versions);
        } catch (err) {
          console.error("获取Optifine版本列表失败:", err);
          setOptifineVersions(LOADER_VERSIONS.optifine ?? []);
        } finally {
          setLoadingOptifine(false);
        }
      };
      fetchOptifineVersions();
    }
  }, [selectedLoader, version.id]);

  // 当选择Fabric加载器时，获取Fabric版本列表
  useEffect(() => {
    if (selectedLoader === "fabric") {
      const fetchFabricVersions = async () => {
        setLoadingFabric(true);
        try {
          // 获取Fabric Loader版本
          const loaderResult = await invoke<{ id: string; version: string }[]>(
            "get_fabric_loader_versions",
            { mcVersion: version.id, useMirror: true }
          );
          const loaderVersions: LoaderVersion[] = loaderResult.map(v => ({
            id: v.id,
            version: v.version,
            releaseDate: "",
            isRecommended: false
          }));
          setFabricLoaderVersions(loaderVersions);

          // 获取Fabric API版本
          const apiResult = await invoke<{ id: string; version: string }[]>(
            "get_fabric_api_versions",
            { mcVersion: version.id }
          );
          const apiVersions: LoaderVersion[] = apiResult.map(v => ({
            id: v.id,
            version: v.version,
            releaseDate: "",
            isRecommended: false
          }));
          setFabricApiVersions(apiVersions);
        } catch (err) {
          console.error("获取Fabric版本列表失败:", err);
          // 如果获取失败，使用默认值
          setFabricLoaderVersions(LOADER_VERSIONS.fabric ?? []);
          setFabricApiVersions([]);
        } finally {
          setLoadingFabric(false);
        }
      };
      fetchFabricVersions();
    }
  }, [selectedLoader, version.id]);

  // 当选择Quilt加载器时，获取Quilt版本列表
  useEffect(() => {
    if (selectedLoader === "quilt") {
      const fetchQuiltVersions = async () => {
        setLoadingQuilt(true);
        try {
          // 获取Quilt Loader版本
          const loaderResult = await invoke<{ id: string; version: string }[]>(
            "get_quilt_loader_versions",
            { mcVersion: version.id }
          );
          const loaderVersions: LoaderVersion[] = loaderResult.map(v => ({
            id: v.id,
            version: v.version,
            releaseDate: "",
            isRecommended: false
          }));
          setQuiltLoaderVersions(loaderVersions);
        } catch (err) {
          console.error("获取Quilt版本列表失败:", err);
          // 如果获取失败，使用默认值
          setQuiltLoaderVersions(LOADER_VERSIONS.quilt ?? []);
        } finally {
          setLoadingQuilt(false);
        }
      };
      fetchQuiltVersions();
    }
  }, [selectedLoader, version.id]);

  // 当选择Forge加载器时，获取Forge版本列表
  useEffect(() => {
    if (selectedLoader === "forge") {
      const fetchForgeVersions = async () => {
        setLoadingForge(true);
        try {
          const result = await invoke<{ id: string; version: string }[]>(
            "get_forge_versions",
            { mcVersion: version.id }
          );
          const versions: LoaderVersion[] = result.map(v => ({
            id: v.id,
            version: v.version,
            releaseDate: "",
            isRecommended: false
          }));
          setForgeVersions(versions);
        } catch (err) {
          console.error("获取Forge版本列表失败:", err);
          setForgeVersions(LOADER_VERSIONS.forge ?? []);
        } finally {
          setLoadingForge(false);
        }
      };
      fetchForgeVersions();
    }
  }, [selectedLoader, version.id]);

  // 当选择NeoForge加载器时，获取NeoForge版本列表
  useEffect(() => {
    if (selectedLoader === "neoforge") {
      const fetchNeoForgeVersions = async () => {
        setLoadingNeoforge(true);
        try {
          const result = await invoke<{ id: string; version: string }[]>(
            "get_neoforge_versions",
            { mcVersion: version.id }
          );
          const versions: LoaderVersion[] = result.map(v => ({
            id: v.id,
            version: v.version,
            releaseDate: "",
            isRecommended: false
          }));
          setNeoforgeVersions(versions);
        } catch (err) {
          console.error("获取NeoForge版本列表失败:", err);
          setNeoforgeVersions(LOADER_VERSIONS.neoforge ?? []);
        } finally {
          setLoadingNeoforge(false);
        }
      };
      fetchNeoForgeVersions();
    }
  }, [selectedLoader, version.id]);

  // 当选择LiteLoader加载器时，获取LiteLoader版本列表
  useEffect(() => {
    if (selectedLoader === "liteloader") {
      const fetchLiteLoaderVersions = async () => {
        setLoadingLiteloader(true);
        try {
          const result = await invoke<{ id: string; version: string }[]>(
            "get_liteloader_versions",
            { mcVersion: version.id }
          );
          const versions: LoaderVersion[] = result.map(v => ({
            id: v.id,
            version: v.version,
            releaseDate: "",
            isRecommended: false
          }));
          setLiteloaderVersions(versions);
        } catch (err) {
          console.error("获取LiteLoader版本列表失败:", err);
          setLiteloaderVersions(LOADER_VERSIONS.liteloader ?? []);
        } finally {
          setLoadingLiteloader(false);
        }
      };
      fetchLiteLoaderVersions();
    }
  }, [selectedLoader, version.id]);

  const promptInstanceNameAndDownload = (info: PendingDownloadInfo) => {
    setPendingDownload(info);
    setInstanceNameInput(info.defaultName);
    setShowNameDialog(true);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 50);
  };

  const confirmDownload = async () => {
    if (!pendingDownload) return;
    const name = instanceNameInput.trim();
    const instanceName = name.length > 0 ? name : pendingDownload.defaultName;

    setShowNameDialog(false);
    const info = pendingDownload;
    setPendingDownload(null);

    if (info.kind === "vanilla") {
      await startDownload(`Minecraft ${version.id}`, version.id, instanceName);
      return;
    }

    const loaderType = info.loaderType!;
    const loaderVersion = info.loaderVersion!;
    try {
      if (loaderType === "optifine") {
        const optifineVersion = loaderVersion.filename || `${loaderVersion.version}.jar`;
        const taskId = await startOptifineDownload(optifineVersion, version.id, instanceName, loaderVersion.official_url);
        console.log(`OptiFine 下载任务已启动，任务ID: ${taskId}`);
      } else if (loaderType === "fabric") {
        const taskId = await startFabricDownload(version.id, loaderVersion.version, undefined, instanceName);
        console.log(`Fabric 下载任务已启动，任务ID: ${taskId}`);
      } else if (loaderType === "quilt") {
        const taskId = await startQuiltDownload(version.id, loaderVersion.version, undefined, instanceName);
        console.log(`Quilt 下载任务已启动，任务ID: ${taskId}`);
      } else if (loaderType === "forge") {
        const taskId = await startForgeDownload(version.id, loaderVersion.version, instanceName);
        console.log(`Forge 下载任务已启动，任务ID: ${taskId}`);
      } else if (loaderType === "neoforge") {
        const taskId = await startNeoForgeDownload(version.id, loaderVersion.version, instanceName);
        console.log(`NeoForge 下载任务已启动，任务ID: ${taskId}`);
      } else if (loaderType === "liteloader") {
        const taskId = await startLiteLoaderDownload(version.id, loaderVersion.version, instanceName);
        console.log(`LiteLoader 下载任务已启动，任务ID: ${taskId}`);
      } else {
        const loaderName =
          LOADER_OPTIONS.find((l) => l.id === loaderType)?.name ?? loaderType;
        startDownload(
          `${version.id} + ${loaderName} ${loaderVersion.version}`,
          version.id,
          instanceName
        );
      }
    } catch (err) {
      console.error("下载失败:", err);
    }
  };

  const handleSelectLoader = (loaderId: LoaderType) => {
    if (loaderId === "vanilla") {
      promptInstanceNameAndDownload({
        kind: "vanilla",
        defaultName: version.id,
      });
      return;
    }
    setSelectedLoader(loaderId);
  };

  const handleInstallLoaderVersion = async (loaderVersion: LoaderVersion) => {
    if (!selectedLoader) return;
    const loaderName =
      LOADER_OPTIONS.find((l) => l.id === selectedLoader)?.name ??
      selectedLoader;
    promptInstanceNameAndDownload({
      kind: "loader",
      loaderType: selectedLoader,
      loaderVersion,
      defaultName: `${version.id}-${loaderName}-${loaderVersion.version}`,
    });
  };

  const selectedLoaderInfo = selectedLoader
    ? LOADER_OPTIONS.find((l) => l.id === selectedLoader)
    : null;

  const loaderVersions = selectedLoader === "optifine"
    ? optifineVersions
    : selectedLoader === "fabric"
    ? fabricLoaderVersions
    : selectedLoader === "quilt"
    ? quiltLoaderVersions
    : selectedLoader === "forge"
    ? forgeVersions
    : selectedLoader === "neoforge"
    ? neoforgeVersions
    : selectedLoader === "liteloader"
    ? liteloaderVersions
    : selectedLoader
    ? LOADER_VERSIONS[selectedLoader] ?? []
    : [];

  return (
    <>
      <div className="flex h-full flex-col gap-4">
      {/* 返回按钮 + 版本信息头 */}
      <div className="flex items-center gap-3 shrink-0">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => {
            if (showFabricApi) {
              setShowFabricApi(false);
            } else if (showQuiltApi) {
              setShowQuiltApi(false);
            } else if (selectedLoader) {
              setSelectedLoader(null);
            } else {
              onBack();
            }
          }}
          aria-label={showFabricApi ? "返回加载器选择" : showQuiltApi ? "返回加载器选择" : selectedLoader ? "返回加载器选择" : "返回版本列表"}
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
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-sm font-medium text-muted-foreground">
                  选择 {selectedLoaderInfo.name} 版本
                </h3>
                <p className="text-xs text-muted-foreground/70 mt-0.5">
                  选择一个 {selectedLoaderInfo.name} 版本进行安装
                </p>
              </div>
              {selectedLoader === "fabric" && (
                <Button
                  variant="outline"
                  size="sm"
                  className="text-xs"
                  onClick={() => setShowFabricApi(true)}
                >
                  查看 Fabric API 版本
                </Button>
              )}
              {selectedLoader === "quilt" && (
                <Button
                  variant="outline"
                  size="sm"
                  className="text-xs"
                  onClick={() => setShowQuiltApi(true)}
                >
                  查看 Quilt API 版本
                </Button>
              )}
            </div>
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
        <AnimatePresence mode="wait">
          {showFabricApi ? (
            <motion.div
              key="fabric-api"
              variants={slideInFromRight}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <FabricApiDetail
                mcVersion={version.id}
                onBack={() => setShowFabricApi(false)}
              />
            </motion.div>
          ) : showQuiltApi ? (
            <motion.div
              key="quilt-api"
              variants={slideInFromRight}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <QuiltApiDetail
                mcVersion={version.id}
                onBack={() => setShowQuiltApi(false)}
              />
            </motion.div>
          ) : selectedLoader && selectedLoaderInfo ? (
            <motion.div
              key="loader-versions"
              variants={slideInFromRight}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <AnimatePresence mode="wait">
                {(loadingOptifine && selectedLoader === "optifine") || (loadingFabric && selectedLoader === "fabric") || (loadingQuilt && selectedLoader === "quilt") || (loadingForge && selectedLoader === "forge") || (loadingNeoforge && selectedLoader === "neoforge") || (loadingLiteloader && selectedLoader === "liteloader") ? (
                  <motion.div
                    key="loading"
                    className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <Loader2 className="size-8 animate-spin" />
                    <p className="text-sm">正在获取版本列表...</p>
                  </motion.div>
                ) : (
                  <motion.div
                    key="list"
                    variants={fadeIn}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                  >
                    <LoaderVersionList
                      loaderName={selectedLoaderInfo.name}
                      versions={loaderVersions}
                      onInstall={handleInstallLoaderVersion}
                    />
                  </motion.div>
                )}
              </AnimatePresence>
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

    {/* 实例名称输入对话框 */}
    <Dialog open={showNameDialog} onOpenChange={setShowNameDialog}>
      <DialogContent className="!max-w-lg p-0">
        <DialogHeader>
          <DialogTitle>
            实例名称
          </DialogTitle>
        </DialogHeader>
        <div className="p-5 space-y-4">
          <div className="space-y-2">
            <p className="text-sm text-muted-foreground">
              请为这个 Minecraft 实例命名，未填写则使用默认名称：
              <code className="mx-1 px-1.5 py-0.5 rounded bg-muted text-xs">
                {pendingDownload?.defaultName}
              </code>
            </p>
            <Input
              ref={inputRef}
              placeholder={pendingDownload?.defaultName}
              value={instanceNameInput}
              onChange={(e) => setInstanceNameInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  confirmDownload();
                }
              }}
            />
          </div>
          <div className="flex items-center justify-end gap-2">
            <Button
              variant="outline"
              onClick={() => {
                setShowNameDialog(false);
                setPendingDownload(null);
              }}
            >
              取消
            </Button>
            <Button onClick={confirmDownload}>
              开始下载
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
    </>
  );
}