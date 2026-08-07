"use client";

import React, { useState, useEffect, useRef, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import {
  Package,
  ArrowLeft,
  UploadCloud,
  Loader2,
  CheckCircle2,
  AlertCircle,
  FileArchive,
  RefreshCw,
  FolderOpen,
  Play,
  Trash2,
  HardDrive,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { fadeSlideUp } from "@/lib/motion";
import { useDownloadManager } from "@/components/download/download-provider";

interface ParsedModpackInfo {
  name: string;
  mc_version: string;
  loader_type: string;
  loader_version: string | null;
  source_file: string;
  file_size: number;
}

interface CachedModpackEntry {
  name: string;
  mc_version: string;
  file_name: string;
  full_path: string;
  file_size: number;
  format: string;
}

type InstallStatus =
  | { kind: "idle" }
  | { kind: "parsing" }
  | { kind: "parsed"; info: ParsedModpackInfo }
  | { kind: "success"; message: string; instanceName: string | null }
  | { kind: "error"; message: string };

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function loaderTypeLabel(t: string): string {
  const map: Record<string, string> = {
    vanilla: "原版",
    forge: "Forge",
    neoforge: "NeoForge",
    fabric: "Fabric",
    quilt: "Quilt",
    liteloader: "LiteLoader",
    optifine: "OptiFine",
  };
  return map[t] || t;
}

function formatLabel(format: string): string {
  const map: Record<string, string> = {
    modrinth: "Modrinth",
    curseforge: "CurseForge",
    unknown: "未知格式",
  };
  return map[format] || format;
}

/**
 * 整合包管理页面
 * 布局：
 *   顶部栏（返回 / 标题 / 刷新）
 *   ├─ 拖放区（支持拖入 .mrpack / .zip 文件）
 *   ├─ 当前选中的整合包详情（解析后的元信息 + 安装按钮）
 *   └─ 已缓存整合包列表（按 MC 版本分组）
 */
export default function ModpackManagementPage() {
  const [cacheList, setCacheList] = useState<CachedModpackEntry[]>([]);
  const [cacheLoading, setCacheLoading] = useState(false);
  const [cacheError, setCacheError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [installStatus, setInstallStatus] = useState<InstallStatus>({ kind: "idle" });
  const [selectedFromCache, setSelectedFromCache] = useState<CachedModpackEntry | null>(null);
  const dropRef = useRef<HTMLDivElement | null>(null);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  const refreshCache = useCallback(async () => {
    setCacheLoading(true);
    setCacheError(null);
    try {
      const list: CachedModpackEntry[] = await invoke("list_cached_modpacks_cmd");
      setCacheList(list || []);
    } catch (e) {
      setCacheError(String(e));
      setCacheList([]);
    } finally {
      setCacheLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshCache();
  }, [refreshCache]);

  // --- Tauri 拖放事件：拿到真实文件路径 ---
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWebviewWindow } = await import(
          "@tauri-apps/api/webviewWindow"
        );
        const w = getCurrentWebviewWindow();
        const fn = await w.onDragDropEvent(async (event) => {
          if (event.payload.type === "over") {
            setDragOver(true);
            return;
          }
          if (event.payload.type === "leave") {
            setDragOver(false);
            return;
          }
          if (event.payload.type === "drop") {
            setDragOver(false);
            const paths = event.payload.paths;
            if (!paths || paths.length === 0) return;
          await acceptFilePath(paths[0]);
          }
        });
        unlisten = fn;
      } catch {
        // 非 Tauri 环境或浏览器 API 不可用
      }
    })();
    return () => {
      unlisten?.();
    };
  }, []);

  // --- 浏览器原生 drag/drop 兜底（在拖放区内部触发） ---
  useEffect(() => {
    const node = dropRef.current;
    if (!node) return;
    const prevent = (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };
    const onEnter = (e: DragEvent) => {
      prevent(e);
      setDragOver(true);
    };
    const onLeave = (e: DragEvent) => {
      prevent(e);
      setDragOver(false);
    };
    const onOver = (e: DragEvent) => {
      prevent(e);
    };
    const onDrop = (e: DragEvent) => {
      prevent(e);
      setDragOver(false);

      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        // File 对象没有真实路径，只能拿到文件名。
        // 此时如果 Tauri onDragDropEvent 已经处理过（通过 paths），
        // 会直接在上面的事件里拿到路径并处理；
        // 如果没拿到（比如纯浏览器环境），这里尝试让用户通过文件选择器选。
        const f = files[0];
        const name = f.name.toLowerCase();
        if (name.endsWith(".mrpack") || name.endsWith(".zip")) {
          pickFileManually();
        }
      }
    };
    node.addEventListener("dragenter", onEnter);
    node.addEventListener("dragleave", onLeave);
    node.addEventListener("dragover", onOver);
    node.addEventListener("drop", onDrop);
    return () => {
      node.removeEventListener("dragenter", onEnter);
      node.removeEventListener("dragleave", onLeave);
      node.removeEventListener("dragover", onOver);
      node.removeEventListener("drop", onDrop);
    };
  }, [installStatus]);

  // 处理一个文件路径
  async function acceptFilePath(path: string) {
    setSelectedFromCache(null);
    setInstallStatus({ kind: "parsing" });
    try {
      const info: ParsedModpackInfo = await invoke("parse_modpack_cmd", {
        path,
      });
      setInstallStatus({ kind: "parsed", info });
    } catch (e) {
      setInstallStatus({ kind: "error", message: String(e) });
    }
  }

  // 手动通过文件选择器选择
  async function pickFileManually() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected: string | null = await open({
        multiple: false,
        directory: false,
        filters: [
          {
            name: "整合包文件",
            extensions: ["mrpack", "zip"],
          },
        ],
      });
      if (selected) {
        await acceptFilePath(selected);
      }
    } catch (e) {
      setInstallStatus({ kind: "error", message: String(e) });
    }
  }

  // 缓存（并安装）整合包
  async function handleCacheAndInstall(path: string) {
    setInstallStatus({ kind: "parsing" });
    try {
      // 先解析
      const info: ParsedModpackInfo = await invoke("parse_modpack_cmd", {
        path,
      });
      // 存到 cache/modpacks/<mc_version>/
      const _cached: string = await invoke("save_modpack_to_cache_cmd", {
        sourcePath: path,
        targetFileName: "",
      });
      setInstallStatus({ kind: "parsed", info });
      await refreshCache();
    } catch (e) {
      setInstallStatus({ kind: "error", message: String(e) });
    }
  }

  // 安装一个整合包（用已解析后的路径或缓存路径）
  const { startModpackDownload } = useDownloadManager();
  async function handleInstall(path: string) {
    try {
      const info: ParsedModpackInfo =
        installStatus.kind === "parsed"
          ? installStatus.info
          : await invoke("parse_modpack_cmd", { path });
      // 启动整合包下载，进度显示在右下角下载任务栏
      await startModpackDownload(info.name, path);
      // 安装成功后，设置成成功状态并刷新缓存
      setInstallStatus({
        kind: "success",
        message: "整合包正在安装中，查看右下角任务栏进度",
        instanceName: null,
      });
      refreshCache();
    } catch (e) {
      setInstallStatus({ kind: "error", message: String(e) });
    }
  }

  // 从缓存列表选择一个
  function handlePickFromCache(entry: CachedModpackEntry) {
    setSelectedFromCache(entry);
    // 模拟解析：把缓存路径当源文件重新解析一次
    acceptFilePath(entry.full_path);
  }

  // 删除一个缓存的整合包压缩包
  async function handleDeleteCached(entry: CachedModpackEntry, e: React.MouseEvent) {
    e.stopPropagation();
    const confirmed = window.confirm(
      `确认删除缓存整合包「${entry.file_name}」？\n\n此操作不可撤销。`
    );
    if (!confirmed) return;
    try {
      await invoke("delete_cached_modpack_cmd", { fullPath: entry.full_path });
      setMessage({ type: "success", text: `已删除 ${entry.file_name}` });
      await refreshCache();
    } catch (err) {
      setMessage({ type: "error", text: String(err) });
    }
  }

  function resetAll() {
    setInstallStatus({ kind: "idle" });
    setSelectedFromCache(null);
    setMessage(null);
  }

  // 按 MC 版本分组
  const groupedByMc: Record<string, CachedModpackEntry[]> = {};
  for (const item of cacheList) {
    if (!groupedByMc[item.mc_version]) groupedByMc[item.mc_version] = [];
    groupedByMc[item.mc_version].push(item);
  }
  const mcVersions = Object.keys(groupedByMc).sort().reverse();

  return (
    <motion.div
      variants={fadeSlideUp}
      initial="initial"
      animate="animate"
      className="flex h-full w-full flex-col"
    >
      {/* 顶部栏 */}
      <div className="shrink-0 flex items-center gap-3 px-3 py-2 border-b bg-background/80 backdrop-blur-sm">
        <a
          href="/game-settings"
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          <ArrowLeft className="size-3.5" />
          <span>返回</span>
        </a>
        <div className="h-4 w-px bg-border" />
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <div className="flex size-8 items-center justify-center rounded-lg bg-muted">
            <Package className="size-4 text-foreground" />
          </div>
          <div className="min-w-0">
            <h1 className="text-sm font-semibold leading-none">整合包管理</h1>
            <p className="text-[11px] text-muted-foreground mt-1 truncate">
              拖入 .mrpack / .zip 整合包，或从缓存中安装
            </p>
          </div>
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="gap-1.5 shrink-0"
          onClick={refreshCache}
          disabled={cacheLoading}
        >
          <RefreshCw className={`size-3.5 ${cacheLoading ? "animate-spin" : ""}`} />
          <span className="text-xs">刷新</span>
        </Button>
      </div>

      {/* 主体内容 */}
      <div className="flex-1 min-h-0 overflow-y-auto p-4 space-y-4">
        {/* 拖放区 */}
        <div
          ref={dropRef}
          className={`relative rounded-2xl border-2 border-dashed transition-all ${
            dragOver
              ? "border-foreground bg-accent/30"
              : "border-border bg-card/30 hover:bg-accent/20"
          } p-8 flex flex-col items-center justify-center gap-3`}
        >
          <div className="flex size-14 items-center justify-center rounded-2xl bg-muted">
            {installStatus.kind === "parsing" ? (
              <Loader2 className="size-7 animate-spin text-foreground" />
            ) : installStatus.kind === "success" ? (
              <CheckCircle2 className="size-7 text-foreground" />
            ) : installStatus.kind === "error" ? (
              <AlertCircle className="size-7 text-foreground" />
            ) : (
              <UploadCloud className="size-7 text-foreground" />
            )}
          </div>

          <div className="text-center">
            <p className="text-sm font-medium">拖入整合包文件到这里</p>
            <p className="text-xs text-muted-foreground mt-1">
              支持 .mrpack (Modrinth) 和 .zip (CurseForge) 格式
            </p>
          </div>

          <div className="flex gap-2 mt-1">
            <Button
              size="sm"
              variant="outline"
              className="gap-1.5"
              onClick={pickFileManually}
            >
              <FolderOpen className="size-3.5" />
              <span className="text-xs">选择文件</span>
            </Button>
            {installStatus.kind !== "idle" && (
              <Button
                size="sm"
                variant="ghost"
                className="gap-1.5"
                onClick={resetAll}
              >
                <span className="text-xs">重置</span>
              </Button>
            )}
          </div>
        </div>

        {/* 状态 / 解析结果 / 安装进度 */}
        <AnimatePresence mode="wait">
          {installStatus.kind === "parsing" && (
            <motion.div
              key="parsing"
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              className="rounded-xl border bg-card/40 p-4 flex items-center gap-3"
            >
              <Loader2 className="size-4 animate-spin text-foreground" />
              <p className="text-sm text-foreground">正在解析整合包……</p>
            </motion.div>
          )}

          {installStatus.kind === "parsed" && (
            <motion.div
              key="parsed"
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              className="rounded-xl border bg-card/40 p-4 space-y-3"
            >
              <div className="flex items-start gap-3">
                <div className="flex size-9 items-center justify-center rounded-lg bg-muted shrink-0">
                  <FileArchive className="size-4.5 text-foreground" />
                </div>
                <div className="flex-1 min-w-0">
                  <h2 className="text-sm font-semibold truncate">
                    {installStatus.info.name}
                  </h2>
                  <div className="flex flex-wrap items-center gap-1.5 mt-1.5">
                    <Badge variant="secondary" className="text-[10px]">
                      MC {installStatus.info.mc_version}
                    </Badge>
                    <Badge variant="secondary" className="text-[10px]">
                      {loaderTypeLabel(installStatus.info.loader_type)}
                      {installStatus.info.loader_version && ` ${installStatus.info.loader_version}`}
                    </Badge>
                    <Badge variant="outline" className="text-[10px]">
                      {formatSize(installStatus.info.file_size)}
                    </Badge>
                  </div>
                  <p className="text-[11px] text-muted-foreground mt-2 break-all">
                    {installStatus.info.source_file}
                  </p>
                </div>
              </div>
              <div className="flex gap-2 pt-1">
                <Button
                  size="sm"
                  variant="default"
                  className="gap-1.5"
                  onClick={() => handleInstall(installStatus.info.source_file)}
                >
                  <Play className="size-3.5" />
                  <span className="text-xs">下载并安装整合包</span>
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  className="gap-1.5"
                  onClick={() => handleCacheAndInstall(installStatus.info.source_file)}
                >
                  <HardDrive className="size-3.5" />
                  <span className="text-xs">保存到缓存</span>
                </Button>
              </div>
            </motion.div>
          )}

          {installStatus.kind === "success" && (
            <motion.div
              key="success"
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              className="rounded-xl border bg-card/40 p-4 flex items-start gap-3"
            >
              <CheckCircle2 className="size-4 text-foreground shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium">{installStatus.message}</p>
                {installStatus.instanceName && (
                  <p className="text-[11px] text-muted-foreground mt-1">
                    已创建实例：{installStatus.instanceName}
                  </p>
                )}
              </div>
              <Button size="sm" variant="ghost" onClick={resetAll}>
                <span className="text-xs">完成</span>
              </Button>
            </motion.div>
          )}

          {installStatus.kind === "error" && (
            <motion.div
              key="error"
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              className="rounded-xl border bg-card/40 p-4 flex items-start gap-3"
            >
              <AlertCircle className="size-4 text-foreground shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium">出现问题</p>
                <p className="text-[11px] text-muted-foreground mt-1 break-all">
                  {installStatus.message}
                </p>
              </div>
              <Button size="sm" variant="ghost" onClick={resetAll}>
                <span className="text-xs">重试</span>
              </Button>
            </motion.div>
          )}
        </AnimatePresence>

        {/* 已缓存的整合包列表 */}
        <div className="space-y-3">
          <div className="flex items-center gap-2">
            <Sparkles className="size-3.5 text-muted-foreground" />
            <h2 className="text-sm font-semibold">已缓存的整合包</h2>
            <span className="text-[11px] text-muted-foreground">
              共 {cacheList.length} 个 · {mcVersions.length} 个 Minecraft 版本
            </span>
          </div>

          {cacheError && (
            <div className="rounded-lg border bg-card/40 p-3 text-[11px] text-muted-foreground">
              加载缓存失败：{cacheError}
            </div>
          )}

          {cacheLoading && (
            <div className="rounded-lg border bg-card/40 p-3 flex items-center gap-2 text-[11px] text-muted-foreground">
              <Loader2 className="size-3 animate-spin" />
              加载中……
            </div>
          )}

          {!cacheLoading && !cacheError && cacheList.length === 0 && (
            <div className="rounded-xl border border-dashed bg-card/20 p-6 text-center">
              <p className="text-xs text-muted-foreground">
                暂无缓存。拖入整合包文件或点击"选择文件"来添加。
              </p>
            </div>
          )}

          <div className="space-y-4">
            {mcVersions.map((mcv) => (
              <div key={mcv} className="space-y-2">
                <div className="flex items-center gap-2 px-1">
                  <Badge variant="secondary" className="text-[10px]">
                    MC {mcv}
                  </Badge>
                  <span className="text-[11px] text-muted-foreground">
                    {groupedByMc[mcv].length} 个整合包
                  </span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2">
                  {groupedByMc[mcv].map((entry) => {
                    const isSelected =
                      selectedFromCache &&
                      selectedFromCache.full_path === entry.full_path;
                    return (
                      <div
                        key={entry.full_path}
                        className={`rounded-xl border bg-card/40 p-3 transition-all hover:bg-accent/20 cursor-pointer ${
                          isSelected
                            ? "border-foreground/60 ring-1 ring-foreground/30"
                            : ""
                        }`}
                        onClick={() => handlePickFromCache(entry)}
                      >
                        <div className="flex items-start gap-2.5">
                          <div className="flex size-8 items-center justify-center rounded-lg bg-muted shrink-0">
                            <FileArchive className="size-4 text-foreground" />
                          </div>
                          <div className="flex-1 min-w-0">
                            <p className="text-xs font-semibold truncate">
                              {entry.name}
                            </p>
                            <p className="text-[10px] text-muted-foreground truncate mt-0.5">
                              {entry.file_name}
                            </p>
                            <div className="flex items-center gap-1 mt-1.5">
                              <Badge variant="outline" className="text-[9px]">
                                {formatLabel(entry.format)}
                              </Badge>
                              <span className="text-[10px] text-muted-foreground tabular-nums">
                                {formatSize(entry.file_size)}
                              </span>
                            </div>
                          </div>
                          <Button
                            size="sm"
                            variant="ghost"
                            className="size-7 shrink-0 p-0"
                            onClick={(e) => {
                              e.stopPropagation();
                              handleInstall(entry.full_path);
                            }}
                            title="安装此整合包"
                          >
                            <Play className="size-3.5" />
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            className="size-7 shrink-0 p-0 hover:text-red-500"
                            onClick={(e) => handleDeleteCached(entry, e)}
                            title="删除此缓存整合包"
                          >
                            <Trash2 className="size-3.5" />
                          </Button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </div>

        {message && (
          <div className="rounded-lg border bg-card/40 p-3 flex items-start gap-2">
            <AlertCircle className="size-3.5 text-foreground shrink-0 mt-0.5" />
            <p className="text-[11px] text-muted-foreground">{message.text}</p>
          </div>
        )}
      </div>
    </motion.div>
  );
}