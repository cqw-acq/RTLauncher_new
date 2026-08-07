"use client";

import type { Dispatch, SetStateAction } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DownloadTask, DownloadTaskStatus } from "./download-provider";

interface ProgressPayload {
  task_id: number;
  percent: number;
}

interface FinishedPayload {
  task_id: number;
  success: boolean;
  error: string | null;
  failed_count?: number;
}

function makeProgressHandler(
  setTasks: Dispatch<SetStateAction<DownloadTask[]>>,
  cancelledRef: { current: boolean }
) {
  return (event: { payload: ProgressPayload }) => {
    if (cancelledRef.current) return;
    const { task_id, percent } = event.payload;
    if (typeof percent !== "number" || Number.isNaN(percent)) return;
    const isCompleted = percent >= 99.99;
    setTasks((prev) => {
      const existing = prev.find((t) => t.taskId === task_id);
      if (existing) {
        return prev.map((task) => {
          if (task.taskId !== task_id) return task;
          if (task.status === "cancelled" || task.status === "success" || task.status === "error" || task.status === "warning") {
            return isCompleted && task.status !== "cancelled" && task.status !== "error"
              ? { ...task, progress: 100 }
              : task;
          }
          return {
            ...task,
            progress: isCompleted ? 100 : percent,
            status: (isCompleted ? "success" : "downloading") as DownloadTaskStatus,
          };
        });
      }
      const newTask: DownloadTask = {
        taskId: task_id,
        label: "Java 运行时",
        mcVersion: "Java",
        status: "downloading",
        progress: isCompleted ? 100 : percent,
        startedAt: Date.now(),
      };
      return [newTask, ...prev];
    });
  };
}

function makeFinishedHandler(
  setTasks: Dispatch<SetStateAction<DownloadTask[]>>,
  cancelledRef: { current: boolean },
  onEnd?: () => void
) {
  return (event: { payload: FinishedPayload }) => {
    if (cancelledRef.current) return;
    const { task_id, success, error, failed_count = 0 } = event.payload;
    setTasks((prev) => {
      const existing = prev.find((t) => t.taskId === task_id);
      if (existing) {
        return prev.map((task) => {
          if (task.taskId !== task_id || task.status === "cancelled") return task;
          const isWarning = success && failed_count > 0;
          return {
            ...task,
            status: (isWarning ? "warning" : success ? "success" : "error") as DownloadTaskStatus,
            progress: success ? 100 : task.progress,
            error: error ?? undefined,
            failedCount: failed_count > 0 ? failed_count : undefined,
          };
        });
      }
      const newTask: DownloadTask = {
        taskId: task_id,
        label: "Java 运行时",
        mcVersion: "Java",
        status: (success ? "success" : "error") as DownloadTaskStatus,
        progress: success ? 100 : 0,
        error: error ?? undefined,
        failedCount: failed_count > 0 ? failed_count : undefined,
        startedAt: Date.now(),
      };
      return [newTask, ...prev];
    });
    onEnd?.();
  };
}

/** 在真正需要下载能力时才注册全部 Tauri 下载事件。 */
export async function setupAllDownloadListeners(
  setTasks: Dispatch<SetStateAction<DownloadTask[]>>,
  dequeueNext: () => void
): Promise<UnlistenFn[]> {
  const cancelledRef = { current: false };
  const progressHandler = makeProgressHandler(setTasks, cancelledRef);
  const finishedHandler = makeFinishedHandler(setTasks, cancelledRef);
  const vanillaFinishedHandler = makeFinishedHandler(
    setTasks,
    cancelledRef,
    dequeueNext
  );
  const eventPairs: Array<[string, string, "vanilla" | "other"]> = [
    ["download-progress", "download-finished", "vanilla"],
    ["java-download-progress", "java-download-finished", "other"],
    ["optifine-download-progress", "optifine-download-finished", "other"],
    ["fabric-download-progress", "fabric-download-finished", "other"],
    ["forge-download-progress", "forge-download-finished", "other"],
    ["mod-download-progress", "mod-download-finished", "other"],
    ["neoforge-download-progress", "neoforge-download-finished", "other"],
    ["liteloader-download-progress", "liteloader-download-finished", "other"],
    ["quilt-download-progress", "quilt-download-finished", "other"],
  ];

  const unlistens: UnlistenFn[] = [];
  for (const [progressEvent, finishedEvent, kind] of eventPairs) {
    unlistens.push(await listen<ProgressPayload>(progressEvent, progressHandler));
    unlistens.push(
      await listen<FinishedPayload>(
        finishedEvent,
        kind === "vanilla" ? vanillaFinishedHandler : finishedHandler
      )
    );
  }

  unlistens.push(await listen<ProgressPayload>("modpack-progress", progressHandler));
  unlistens.push(
    await listen("modpack-finished", (event) => {
      const payload = event.payload as {
        task_id: number;
        success: boolean;
        message: string;
      };
      setTasks((prev) =>
        prev.map((task) =>
          task.taskId === payload.task_id && task.status !== "cancelled"
            ? {
                ...task,
                status: (payload.success ? "success" : "error") as DownloadTaskStatus,
                progress: payload.success ? 100 : task.progress,
                error: payload.success ? undefined : String(payload.message),
              }
            : task
        )
      );
    })
  );
  return unlistens;
}