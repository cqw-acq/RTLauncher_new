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
        setTasks((prev) =>
          prev.map((t) =>
            t.taskId === taskId ? { ...t, taskId: returnedTaskId } : t
          )
        );
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