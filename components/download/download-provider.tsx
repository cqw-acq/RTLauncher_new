"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { type UnlistenFn } from "@tauri-apps/api/event";
import { makeStartDownloadFn } from "./download-event-utils";
import { isTauriRuntime } from "@/lib/tauri-runtime";

export type DownloadTaskStatus =
  | "queued"
  | "downloading"
  | "success"
  | "warning"
  | "error"
  | "cancelled";

export interface DownloadTask {
  taskId: number;
  label: string;
  mcVersion: string;
  status: DownloadTaskStatus;
  progress?: number;
  error?: string;
  /** 部分失败的文件数量 */
  failedCount?: number;
  /** 时间戳，用于排序 */
  startedAt: number;
}

interface DownloadContextValue {
  tasks: DownloadTask[];
  /** 启动一个原版下载任务（排队制）*/
  startDownload: (label: string, mcVersion: string, instanceName?: string) => Promise<number>;
  /** 启动 Java 下载（不排队）*/
  startJavaDownload: (runtimeName: string) => Promise<number>;
  /** 启动 OptiFine 下载（不排队）*/
  startOptifineDownload: (optifineVersion: string, mcVersion: string, instanceName?: string, optifineFallbackUrl?: string) => Promise<number>;
  /** 启动 Fabric 下载（不排队）*/
  startFabricDownload: (mcVersion: string, loaderVersion: string, apiVersion?: string, instanceName?: string) => Promise<number>;
  /** 启动 Quilt 下载（不排队）*/
  startQuiltDownload: (mcVersion: string, loaderVersion: string, apiVersion?: string, instanceName?: string) => Promise<number>;
  /** 启动 Forge 下载（不排队）*/
  startForgeDownload: (mcVersion: string, forgeVersion: string, instanceName?: string) => Promise<number>;
  /** 启动 NeoForge 下载（不排队）*/
  startNeoForgeDownload: (mcVersion: string, neoforgeVersion: string, instanceName?: string) => Promise<number>;
  /** 启动 LiteLoader 下载（不排队）*/
  startLiteLoaderDownload: (mcVersion: string, liteloaderVersion: string, instanceName?: string) => Promise<number>;
  /** Mod 文件下载 */
  startModDownload: (modSlug: string, modName: string, mcVersion: string, modLoader: string, downloadUrl: string) => Promise<number>;
  /** 通用资源文件下载（mod / resourcepack / shaderpack / datapack / world） */
  startResourceDownload: (resourceKind: string, resourceSlug: string, resourceName: string, mcVersion: string, modLoader: string, downloadUrl: string) => Promise<number>;
  /** 整合包安装（异步） */
  startModpackDownload: (modpackName: string, path: string) => Promise<number>;
  /** 取消一个下载任务 */
  cancelDownload: (taskId: number) => Promise<void>;
  /** 清除所有已完成/失败/取消的任务 */
  clearFinished: () => void;
  /** 从任务列表中移除单个任务 */
  removeTask: (taskId: number) => void;
}

const DownloadContext = createContext<DownloadContextValue | null>(null);

export function useDownloadManager() {
  const ctx = useContext(DownloadContext);
  if (!ctx) {
    throw new Error("useDownloadManager must be used within DownloadProvider");
  }
  return ctx;
}

/** 排队中的任务（还未发往后端，只用于原版下载）*/
interface QueueItem {
  localId: number;
  label: string;
  mcVersion: string;
  instanceName?: string;
}

export function DownloadProvider({ children }: { children: React.ReactNode }) {
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const unlistensRef = useRef<UnlistenFn[]>([]);
  const listenerSetupRef = useRef<Promise<void> | null>(null);
  const listenerOwnerActiveRef = useRef(true);
  const pendingQueueRef = useRef<QueueItem[]>([]);
  const isDownloadingRef = useRef(false);
  const localIdCounterRef = useRef(-1);
  /** 非原版下载的 taskId 计数器（从 1_000_000 起，避免与后端 taskId 冲突）*/
  const taskIdCounterRef = useRef(1_000_000);

  /** ========== 排队机制（仅用于原版 Minecraft 下载）========== */
  const dequeueNext = useCallback(async () => {
    if (isDownloadingRef.current) return;
    const next = pendingQueueRef.current.shift();
    if (!next) return;

    isDownloadingRef.current = true;
    try {
      const taskId = await invoke<number>("download_patcher", {
        mcVersion: next.mcVersion,
        instanceName: next.instanceName ?? null,
      });
      setTasks((prev) =>
        prev.map((t) =>
          t.taskId === next.localId
            ? { ...t, taskId, status: "downloading" as const, startedAt: Date.now() }
            : t
        )
      );
    } catch (e) {
      setTasks((prev) =>
        prev.map((t) =>
          t.taskId === next.localId
            ? { ...t, status: "error" as const, error: String(e) }
            : t
        )
      );
      isDownloadingRef.current = false;
      dequeueNext();
    }
  }, []);

  /** 标记下载已结束，释放排队锁并尝试启动下一个任务 */
  const markDownloadDone = useCallback(() => {
    isDownloadingRef.current = false;
    void dequeueNext();
  }, [dequeueNext]);

  /** 首次下载前确保监听已经完成注册，空闲时会提前预热。 */
  const ensureDownloadListeners = useCallback(async () => {
    if (!isTauriRuntime() || unlistensRef.current.length > 0) return;
    if (!listenerSetupRef.current) {
      listenerSetupRef.current = import("./download-listeners")
        .then(({ setupAllDownloadListeners }) =>
          setupAllDownloadListeners(setTasks, markDownloadDone)
        )
        .then((unlistens) => {
          if (!listenerOwnerActiveRef.current) {
            unlistens.forEach((unlisten) => unlisten());
            listenerSetupRef.current = null;
            return;
          }
          unlistensRef.current = unlistens;
        })
        .catch((error) => {
          listenerSetupRef.current = null;
          throw error;
        });
    }
    await listenerSetupRef.current;
  }, [markDownloadDone]);

  /** ========== 空闲时预热事件监听 ========== */
  useEffect(() => {
    if (!isTauriRuntime()) return;

    let cancelled = false;
    listenerOwnerActiveRef.current = true;
    const warmUp = () => {
      void ensureDownloadListeners().catch((error) => {
        if (!cancelled) console.error("注册下载事件监听失败:", error);
      });
    };
    const timeoutId = window.setTimeout(warmUp, 0);

    return () => {
      cancelled = true;
      listenerOwnerActiveRef.current = false;
      window.clearTimeout(timeoutId);
      unlistensRef.current.forEach((fn) => fn());
      unlistensRef.current = [];
      listenerSetupRef.current = null;
    };
  }, [ensureDownloadListeners]);

  /** ========== 启动函数 ========== */

  /** 原版下载（排队制）*/
  const startDownload = useCallback(
    async (label: string, mcVersion: string, instanceName?: string): Promise<number> => {
      await ensureDownloadListeners();
      if (!isDownloadingRef.current) {
        isDownloadingRef.current = true;
        try {
          const taskId = await invoke<number>("download_patcher", { mcVersion, instanceName: instanceName ?? null });
          const task: DownloadTask = {
            taskId,
            label,
            mcVersion,
            status: "downloading",
            startedAt: Date.now(),
          };
          setTasks((prev) => [task, ...prev]);
          return taskId;
        } catch (e) {
          isDownloadingRef.current = false;
          throw e;
        }
      } else {
        const localId = localIdCounterRef.current--;
        pendingQueueRef.current.push({ localId, label, mcVersion, instanceName });
        const task: DownloadTask = {
          taskId: localId,
          label,
          mcVersion,
          status: "queued",
          startedAt: Date.now(),
        };
        setTasks((prev) => [task, ...prev]);
        return localId;
      }
    },
    [ensureDownloadListeners]
  );

  /** 使用工厂生成的通用 start 函数 */
  const startModLoaderDownload = makeStartDownloadFn(
    setTasks,
    taskIdCounterRef,
    dequeueNext,
    ensureDownloadListeners
  );

  /** Java 下载 - 只创建前端 task，不 invoke，Java 下载由其他命令触发 */
  const startJavaDownload = useCallback(async (runtimeName: string) => {
    await ensureDownloadListeners();
    const taskId = taskIdCounterRef.current++;
    setTasks((prev) => {
      const isDownloading = prev.some(
        (t) => t.label === runtimeName && (t.status === "downloading" || t.status === "success")
      );
      if (isDownloading) return prev;
      const task: DownloadTask = {
        taskId,
        label: runtimeName,
        mcVersion: "Java",
        status: "downloading",
        startedAt: Date.now(),
      };
      return [task, ...prev];
    });
    return taskId;
  }, [ensureDownloadListeners]);

  /** OptiFine */
  const startOptifineDownload = useCallback(
    async (optifineVersion: string, mcVersion: string, instanceName?: string, optifineFallbackUrl?: string) => {
      return startModLoaderDownload({
        label: optifineVersion,
        mcVersion,
        tauriCommand: "download_and_install_optifine",
        params: { optifineVersion, mcVersion, instanceName: instanceName ?? null, optifineFallbackUrl: optifineFallbackUrl ?? null },
      });
    },
    [startModLoaderDownload]
  );

  /** Fabric */
  const startFabricDownload = useCallback(
    async (mcVersion: string, loaderVersion: string, apiVersion?: string, instanceName?: string) => {
      const label = `Fabric ${loaderVersion}${apiVersion ? ` + API ${apiVersion}` : ""}`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_and_install_fabric",
        params: { mcVersion, loaderVersion, apiVersion: apiVersion || null, instanceName: instanceName ?? null },
      });
    },
    [startModLoaderDownload]
  );

  /** Quilt */
  const startQuiltDownload = useCallback(
    async (mcVersion: string, loaderVersion: string, apiVersion?: string, instanceName?: string) => {
      const label = `Quilt ${loaderVersion}${apiVersion ? ` + API ${apiVersion}` : ""}`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_and_install_quilt",
        params: { mcVersion, loaderVersion, apiVersion: apiVersion || null, instanceName: instanceName ?? null },
      });
    },
    [startModLoaderDownload]
  );

  /** Forge */
  const startForgeDownload = useCallback(
    async (mcVersion: string, forgeVersion: string, instanceName?: string) => {
      const label = `Forge ${forgeVersion} (MC ${mcVersion})`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_and_install_forge",
        params: { mcVersion, forgeVersion, instanceName: instanceName ?? null },
      });
    },
    [startModLoaderDownload]
  );

  /** NeoForge */
  const startNeoForgeDownload = useCallback(
    async (mcVersion: string, neoforgeVersion: string, instanceName?: string) => {
      const label = `NeoForge ${neoforgeVersion} (MC ${mcVersion})`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_and_install_neoforge",
        params: { mcVersion, neoforgeVersion, instanceName: instanceName ?? null },
      });
    },
    [startModLoaderDownload]
  );

  /** LiteLoader */
  const startLiteLoaderDownload = useCallback(
    async (mcVersion: string, liteloaderVersion: string, instanceName?: string) => {
      const label = `LiteLoader ${liteloaderVersion} (MC ${mcVersion})`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_and_install_liteloader",
        params: { mcVersion, liteloaderVersion, instanceName: instanceName ?? null },
      });
    },
    [startModLoaderDownload]
  );

  /** Mod 文件下载 */
  const startModDownload = useCallback(
    async (modSlug: string, modName: string, mcVersion: string, modLoader: string, downloadUrl: string) => {
      const label = `${modName} (${mcVersion})`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_mod_file",
        params: { modSlug, modName, mcVersion, modLoader, downloadUrl },
      });
    },
    [startModLoaderDownload]
  );

  /** 通用资源文件下载（mod / resourcepack / shaderpack / datapack / world） */
  const startResourceDownload = useCallback(
    async (resourceKind: string, resourceSlug: string, resourceName: string, mcVersion: string, modLoader: string, downloadUrl: string) => {
      const label = `${resourceName} (${mcVersion})`;
      return startModLoaderDownload({
        label,
        mcVersion,
        tauriCommand: "download_resource_file",
        params: { resourceKind, resourceSlug, resourceName, mcVersion, modLoader, downloadUrl },
      });
    },
    [startModLoaderDownload]
  );

  /** 整合包安装（异步，进度显示在右下角下载任务栏） */
  const startModpackDownload = useCallback(
    async (modpackName: string, path: string) => {
      const label = `整合包: ${modpackName}`;
      return startModLoaderDownload({
        label,
        mcVersion: "整合包",
        tauriCommand: "install_modpack_from_zip_cmd",
        params: { path },
      });
    },
    [startModLoaderDownload]
  );

  /** ========== 取消与清理 ========== */
  const cancelDownload = useCallback(
    async (taskId: number) => {
      if (taskId < 0) {
        // 排队中的任务，从队列和 tasks 中移除即可
        pendingQueueRef.current = pendingQueueRef.current.filter(
          (item) => item.localId !== taskId
        );
        setTasks((prev) => prev.filter((t) => t.taskId !== taskId));
        return;
      }
      // 立即更新前端状态
      setTasks((prev) =>
        prev.map((t) =>
          t.taskId === taskId && t.status === "downloading"
            ? { ...t, status: "cancelled" as const, error: "已取消" }
            : t
        )
      );
      try {
        await Promise.allSettled([
          invoke("cancel_download", { taskId }),
          invoke("cancel_mod_download", { taskId }),
          invoke("cancel_optifine_download", { taskId }),
          invoke("cancel_fabric_download", { taskId }),
          invoke("cancel_quilt_download", { taskId }),
          invoke("cancel_forge_download", { taskId }),
          invoke("cancel_neoforge_download", { taskId }),
          invoke("cancel_liteloader_download", { taskId }),
        ]);
      } catch (e) {
        console.error("取消下载失败:", e);
      }
      // 取消正在下载的任务后，重置下载状态并尝试启动下一个任务
      isDownloadingRef.current = false;
      dequeueNext();
    },
    [dequeueNext]
  );

  const clearFinished = useCallback(() => {
    setTasks((prev) =>
      prev.filter((t) => t.status === "downloading" || t.status === "queued")
    );
  }, []);

  const removeTask = useCallback((taskId: number) => {
    setTasks((prev) => prev.filter((t) => t.taskId !== taskId));
  }, []);

  return (
    <DownloadContext.Provider
      value={{
        tasks,
        startDownload,
        startJavaDownload,
        startOptifineDownload,
        startFabricDownload,
        startQuiltDownload,
        startForgeDownload,
        startNeoForgeDownload,
        startLiteLoaderDownload,
        startModDownload,
        startResourceDownload,
        startModpackDownload,
        cancelDownload,
        clearFinished,
        removeTask,
      }}
    >
      {children}
    </DownloadContext.Provider>
  );
}