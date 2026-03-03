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
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DownloadTaskStatus =
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
  progress: number;
  error?: string;
  /** 部分失败的文件数量 */
  failedCount?: number;
  /** 时间戳，用于排序 */
  startedAt: number;
}

interface DownloadContextValue {
  tasks: DownloadTask[];
  /** 启动一个下载任务，返回 taskId */
  startDownload: (label: string, mcVersion: string) => Promise<number>;
  /** 取消下载任务（后端会清理已下载文件） */
  cancelDownload: (taskId: number) => Promise<void>;
  /** 清除已完成/失败的任务 */
  clearFinished: () => void;
  /** 清除指定任务 */
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

export function DownloadProvider({ children }: { children: React.ReactNode }) {
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const unlistensRef = useRef<UnlistenFn[]>([]);

  // 监听后端事件
  useEffect(() => {
    let cancelled = false;

    async function setup() {
      const unlistenProgress = await listen<{ task_id: number; percent: number }>(
        "download-progress",
        (event) => {
          if (cancelled) return;
          const { task_id, percent } = event.payload;
          setTasks((prev) =>
            prev.map((t) =>
              t.taskId === task_id && t.status === "downloading"
                ? { ...t, progress: percent }
                : t
            )
          );
        }
      );

      const unlistenFinished = await listen<{
        task_id: number;
        success: boolean;
        error: string | null;
        failed_count: number;
      }>("download-finished", (event) => {
        if (cancelled) return;
        const { task_id, success, error, failed_count } = event.payload;
        setTasks((prev) =>
          prev.map((t) => {
            if (t.taskId !== task_id) return t;
            // 已取消的任务忽略后端事件
            if (t.status === "cancelled") return t;
            // 部分成功：success=true 但有文件失败
            const isWarning = success && failed_count > 0;
            return {
              ...t,
              status: isWarning ? "warning" : success ? "success" : "error",
              progress: success ? 100 : t.progress,
              error: error ?? undefined,
              failedCount: failed_count > 0 ? failed_count : undefined,
            };
          })
        );
      });

      unlistensRef.current = [unlistenProgress, unlistenFinished];
    }

    setup();

    return () => {
      cancelled = true;
      unlistensRef.current.forEach((fn) => fn());
      unlistensRef.current = [];
    };
  }, []);

  const startDownload = useCallback(
    async (label: string, mcVersion: string): Promise<number> => {
      const taskId = await invoke<number>("download_patcher", { mcVersion });

      const task: DownloadTask = {
        taskId,
        label,
        mcVersion,
        status: "downloading",
        progress: 0,
        startedAt: Date.now(),
      };

      setTasks((prev) => [task, ...prev]);
      return taskId;
    },
    []
  );

  const cancelDownload = useCallback(async (taskId: number) => {
    // 立即更新前端状态
    setTasks((prev) =>
      prev.map((t) =>
        t.taskId === taskId && t.status === "downloading"
          ? { ...t, status: "cancelled" as const, error: "已取消" }
          : t
      )
    );
    try {
      await invoke("cancel_download", { taskId });
    } catch (e) {
      console.error("取消下载失败:", e);
    }
  }, []);

  const clearFinished = useCallback(() => {
    setTasks((prev) => prev.filter((t) => t.status === "downloading"));
  }, []);

  const removeTask = useCallback((taskId: number) => {
    setTasks((prev) => prev.filter((t) => t.taskId !== taskId));
  }, []);

  return (
    <DownloadContext.Provider
      value={{ tasks, startDownload, cancelDownload, clearFinished, removeTask }}
    >
      {children}
    </DownloadContext.Provider>
  );
}
