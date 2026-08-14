"use client";

import type { Dispatch, SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DownloadTask } from "./download-provider";

interface StartDownloadOptions {
  /** 用于生成 label，显示在 UI 上 */
  label: string;
  /** Minecraft 版本，用于在任务列表显示 */
  mcVersion: string;
  /** 后端 Tauri 命令名称，如 "download_and_install_forge" */
  tauriCommand: string;
  /** 传给后端的参数对象 */
  params: Record<string, unknown>;
}

/**
 * 生成一个 startXXXDownload 函数的工厂
 *
 * 所有下载器（Forge / NeoForge / Fabric / Quilt / OptiFine / Mod 都走这套逻辑
 * 只有原版下载不一样（它走队列）
 */
export function makeStartDownloadFn(
  setTasks: Dispatch<SetStateAction<DownloadTask[]>>,
  taskIdCounterRef: { current: number },
  dequeueNext?: () => void,
  ensureListeners?: () => Promise<void>
) {
  return async function startDownload(opts: StartDownloadOptions): Promise<number> {
    const taskId = taskIdCounterRef.current++;
    const { label, mcVersion, tauriCommand, params } = opts;

    setTasks((prev) => {
      const isDownloading = prev.some(
        (t) => t.label === label && (t.status === "downloading" || t.status === "success")
      );
      if (isDownloading) return prev;

      const task: DownloadTask = {
        clientId: `client-${taskId}`,
        taskId,
        label,
        mcVersion,
        status: "downloading",
        startedAt: Date.now(),
      };
      return [task, ...prev];
    });

    try {
      await ensureListeners?.();
      const returnedTaskId = await invoke<number>(tauriCommand, params);
      // 如果后端返回的 taskId 与我们生成的不一样，替换掉
      if (returnedTaskId !== taskId) {
        setTasks((prev) => {
          const localTask = prev.find((task) => task.taskId === taskId);
          if (!localTask) return prev;

          // 进度事件可能在 invoke 返回前到达。合并该事件创建的占位任务，
          // 保留前端 clientId，避免同一个后端任务在列表中出现两次。
          const eventTask = prev.find(
            (task) => task.taskId === returnedTaskId && task.clientId !== localTask.clientId,
          );
          return prev.flatMap((task) => {
            if (task === eventTask) return [];
            if (task !== localTask) return [task];
            return [{
              ...localTask,
              taskId: returnedTaskId,
              status: eventTask?.status ?? localTask.status,
              progress: eventTask?.progress ?? localTask.progress,
              error: eventTask?.error ?? localTask.error,
              failedCount: eventTask?.failedCount ?? localTask.failedCount,
            }];
          });
        });
      }
      return returnedTaskId;
    } catch (err) {
      console.error(`启动下载失败 (${tauriCommand}):`, err);
      // 标记任务为失败
      setTasks((prev) =>
        prev.map((t) =>
          t.taskId === taskId ? { ...t, status: "error", error: String(err) } : t
        )
      );
      // 启动失败后尝试下一个排队任务
      dequeueNext?.();
      throw err;
    }
  };
}
