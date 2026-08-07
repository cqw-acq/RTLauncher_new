"use client";

import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  X,
  UploadCloud,
  Loader2,
  CheckCircle2,
  AlertCircle,
  Package,
  FolderOpen,
  FileArchive,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

// --------------------------------------------------------------------------
// 后端类型映射
// --------------------------------------------------------------------------

type ModpackDetectResult = {
  format: string; // "modrinth" | "curseforge" | "unknown"
  recognizable: boolean;
};

type ModpackProgressPayload = {
  task_id: number;
  percent: number;
  stage: string;
  total_files: number;
  downloaded_files: number;
  current_file: string;
};

type ModpackFinishedPayload = {
  task_id: number;
  success: boolean;
  message: string;
  instance_name?: string;
  file_count?: number;
};

// --------------------------------------------------------------------------
// 状态
// --------------------------------------------------------------------------

type DialogStatus =
  | "idle"          // 未安装/等待拖入
  | "detecting"     // 检测格式中
  | "ready"         // 可识别，等待用户点击"开始安装"
  | "installing"    // 安装中
  | "success"       // 安装完成
  | "error";        // 失败

// --------------------------------------------------------------------------
// 组件：整合包安装对话框
// --------------------------------------------------------------------------

type Props = {
  open: boolean;
  onClose: () => void;
};

export function ModpackInstallerDialog({ open, onClose }: Props) {
  const [status, setStatus] = useState<DialogStatus>("idle");
  const [errorMsg, setErrorMsg] = useState<string>("");
  const [zipPath, setZipPath] = useState<string>("");
  const [zipName, setZipName] = useState<string>("");
  const [format, setFormat] = useState<string>("");

  const [taskId, setTaskId] = useState<number | null>(null);
  const [percent, setPercent] = useState<number>(0);
  const [stage, setStage] = useState<string>("初始化…");
  const [totalFiles, setTotalFiles] = useState<number>(0);
  const [downloadedFiles, setDownloadedFiles] = useState<number>(0);
  const [currentFile, setCurrentFile] = useState<string>("");

  const [result, setResult] = useState<ModpackFinishedPayload | null>(null);
  const dropRef = useRef<HTMLDivElement>(null);
  const [dragOver, setDragOver] = useState(false);

  // 每次打开重置
  useEffect(() => {
    if (open) {
      setStatus("idle");
      setErrorMsg("");
      setZipPath("");
      setZipName("");
      setFormat("");
      setTaskId(null);
      setPercent(0);
      setStage("初始化…");
      setTotalFiles(0);
      setDownloadedFiles(0);
      setCurrentFile("");
      setResult(null);
    }
  }, [open]);

  // --- 浏览器原生 drag & drop：仅在对话框内的拖放区域处理 ---
  useEffect(() => {
    if (!open) return;

    const prevent = (e: DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    const onEnter = (e: DragEvent) => {
      prevent(e);
      if (status === "idle" || status === "ready" || status === "error") {
        setDragOver(true);
      }
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
      if (status === "installing" || status === "success") return;

      // 先尝试浏览器原生 dataTransfer
      const files = e.dataTransfer?.files;
      if (files && files.length > 0) {
        const f = files[0];
        const name = f.name;
        // File 对象没有真实路径，我们只能靠名字识别扩展名，
        // 再让用户用文件选择器（无法通过拖放拿到绝对路径）
        if (!/\.(zip|mrpack|jar)$/i.test(name)) {
          setStatus("error");
          setErrorMsg(`无法识别的文件：${name}（请拖入 .zip / .mrpack 整合包）`);
          return;
        }
        // 在 Tauri 环境下，尝试读取 dragged file 的真实路径
        // Web 版 File 没有 path，所以这里给出提示：在启动器窗口内拖入时，
        // 会由 Tauri 的原生拖放事件拿到真实路径；如果是浏览器里直接丢过来的，
        // 会回退到文件选择按钮。
        // 因此我们直接触发 detect：用一个隐藏的 <input type=file> 或者
        // 通过 webview 原生事件拿到路径。
        //
        // 策略：先尝试从 Tauri 原生事件里拿路径（见下方 webviewDragHandler）；
        // 如果拿不到但有文件名，则提示用户改用"选择文件"按钮。
        setStatus("error");
        setErrorMsg(`已识别文件名：${name}，请改用下方"选择文件"按钮选择（浏览器拖放不提供真实路径）。`);
        return;
      }

      // Tauri 原生拖拽事件的路径会写到 dataTransfer 的 types/items，
      // 这里做兜底：尝试通过 invoke 打开文件选择器
      pickFileManually();
    };

    const node = dropRef.current;
    if (!node) return;

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
  }, [open, status]);

  // --- Tauri 原生 drag & drop：拿到真实文件路径 ---
  useEffect(() => {
    if (!open) return;
    let unlisten: (() => void) | undefined;

    (async () => {
      try {
        const { getCurrentWebviewWindow } = await import(
          "@tauri-apps/api/webviewWindow"
        );
        const w = getCurrentWebviewWindow();
        const fn = await w.onDragDropEvent(async (event) => {
          if (status === "installing" || status === "success") return;
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
        // 非 Tauri 环境忽略
      }
    })();

    return () => {
      unlisten?.();
    };
  }, [open, status]);

  // --- Tauri 事件：modpack-progress / modpack-finished ---
  useEffect(() => {
    if (!open) return;
    let unlistenProgress: (() => void) | undefined;
    let unlistenFinished: (() => void) | undefined;

    (async () => {
      try {
        const { getCurrentWebviewWindow } = await import(
          "@tauri-apps/api/webviewWindow"
        );
        const w = getCurrentWebviewWindow();

        unlistenProgress = await w.listen<ModpackProgressPayload>(
          "modpack-progress",
          (event) => {
            const p = event.payload;
            setPercent(p.percent);
            setStage(p.stage);
            setTotalFiles(p.total_files ?? 0);
            setDownloadedFiles(p.downloaded_files ?? 0);
            setCurrentFile(p.current_file ?? "");
            // 首次收到进度事件时确认 taskId
            if (taskId === null && p.task_id !== undefined && p.task_id !== null) {
              setTaskId(p.task_id);
            }
          },
        );

        unlistenFinished = await w.listen<ModpackFinishedPayload>(
          "modpack-finished",
          (event) => {
            const p = event.payload;
            setResult(p);
            setStatus(p.success ? "success" : "error");
            if (!p.success) {
              setErrorMsg(p.message ?? "未知错误");
            }
          },
        );
      } catch {
        // 非 Tauri 环境忽略
      }
    })();

    return () => {
      unlistenProgress?.();
      unlistenFinished?.();
    };
  }, [open, taskId]);

  // --- 用原生 input file 选择（兜底方案）---
  const pickFileManually = () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".zip,.mrpack";
    input.onchange = async () => {
      const f = input.files?.[0];
      if (!f) return;
      // File.path 在 Tauri 环境下会返回真实路径
      const file = f as File & { path?: string };
      const p = file.path || (file as any).webkitRelativePath || "";
      if (p) {
        await acceptFilePath(p);
      } else {
        setStatus("error");
        setErrorMsg("无法获取文件真实路径，请尝试从启动器外部拖入或改用其他方式。");
      }
    };
    input.click();
  };

  // --- 拿到真实路径后：检测 → 安装 ---
  const acceptFilePath = async (path: string) => {
    const name = path.split(/[\\/]/).pop() || path;
    setZipPath(path);
    setZipName(name);
    setStatus("detecting");
    setErrorMsg("");
    try {
      const r = await invoke<ModpackDetectResult>("detect_modpack_format_cmd", {
        path,
      });
      if (r.recognizable) {
        setFormat(r.format);
        setStatus("ready");
      } else {
        setStatus("error");
        setErrorMsg(`无法识别的整合包：${name}`);
      }
    } catch (e) {
      setStatus("error");
      setErrorMsg(`检测失败：${String(e)}`);
    }
  };

  const startInstall = async () => {
    if (!zipPath) return;
    setStatus("installing");
    setPercent(0);
    setStage("初始化…");
    setResult(null);
    try {
      const tid = await invoke<number>("install_modpack_from_zip_cmd", {
        path: zipPath,
      });
      setTaskId(tid);
    } catch (e) {
      console.error("[ModpackInstaller] install invoke error:", e);
      setStatus("error");
      setErrorMsg(`安装调用失败：${String(e)}`);
    }
  };

  const cancelInstall = async () => {
    if (taskId !== null) {
      try {
        await invoke("cancel_modpack_install", { taskId: taskId });
      } catch {
        /* ignore */
      }
    }
    setStatus("idle");
  };

  // --------------------------------------------------------------------------
  // 渲染
  // --------------------------------------------------------------------------

  if (!open) return null;

  // 显示的百分比（按文件数更友好）
  const percentText = totalFiles > 0 && downloadedFiles > 0
    ? `${downloadedFiles} / ${totalFiles} 个文件 (${Math.min(100, Math.round(percent))}%)`
    : `${Math.min(100, Math.round(percent))}%`;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* 遮罩 */}
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />

      {/* 对话框 */}
      <div className="relative z-10 w-full max-w-xl mx-4 rounded-2xl bg-background border shadow-2xl overflow-hidden">
        {/* 头部 */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border">
          <div className="flex items-center gap-2">
            <div className="flex size-9 items-center justify-center rounded-lg bg-primary/10">
              <Package className="size-5 text-primary" />
            </div>
            <div>
              <h3 className="text-base font-semibold">安装整合包</h3>
              <p className="text-xs text-muted-foreground mt-0.5">
                将整合包拖入下方区域，或点击"选择文件"
              </p>
            </div>
          </div>
          <button
            type="button"
            className={cn(
              "size-8 flex items-center justify-center rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors",
              status === "installing" && "pointer-events-none opacity-50",
            )}
            onClick={onClose}
            disabled={status === "installing"}
          >
            <X className="size-4" />
          </button>
        </div>

        {/* 主体 */}
        <div className="p-6 space-y-4">
          {/* 拖放区域 */}
          <div
            ref={dropRef}
            className={cn(
              "flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed px-6 py-10 text-center transition-all duration-200",
              dragOver
                ? "border-primary bg-primary/5 text-primary scale-[1.01]"
                : status === "success"
                ? "border-green-500 bg-green-500/5"
                : status === "error"
                ? "border-destructive bg-destructive/5"
                : "border-border text-muted-foreground hover:border-primary/50 hover:bg-muted/30",
              status === "installing" && "pointer-events-none opacity-70",
            )}
          >
            {status === "installing" ? (
              <>
                <Loader2 className="size-8 animate-spin text-primary" />
                <p className="text-sm font-medium text-foreground">正在安装…</p>
                <p className="text-xs text-muted-foreground max-w-md truncate">
                  {currentFile || stage || "请稍候"}
                </p>
              </>
            ) : status === "success" ? (
              <>
                <CheckCircle2 className="size-8 text-green-500" />
                <p className="text-sm font-medium text-green-600 dark:text-green-400">
                  安装完成
                </p>
                {result?.instance_name && (
                  <p className="text-xs text-muted-foreground font-mono">
                    实例：{result.instance_name}
                  </p>
                )}
              </>
            ) : status === "detecting" ? (
              <>
                <Loader2 className="size-8 animate-spin text-primary" />
                <p className="text-sm font-medium">检测中…</p>
              </>
            ) : (
              <>
                <UploadCloud
                  className={cn(
                    "size-10 transition-transform",
                    dragOver && "text-primary scale-110",
                  )}
                />
                <div className="space-y-1">
                  <p className="text-sm font-medium text-foreground">
                    将整合包文件拖到此处
                  </p>
                  <p className="text-xs text-muted-foreground">
                    支持 Modrinth (.mrpack) 与 CurseForge (.zip) 格式
                  </p>
                </div>
              </>
            )}
          </div>

          {/* 文件选择按钮（当拖放无效时的兜底） */}
          {status !== "installing" && status !== "success" && (
            <div className="flex justify-center">
              <Button
                variant="ghost"
                size="sm"
                onClick={pickFileManually}
                className="gap-2 text-xs"
              >
                <FolderOpen className="size-3.5" />
                或选择文件…
              </Button>
            </div>
          )}

          {/* 识别结果/错误 */}
          {status === "ready" && (
            <div className="flex items-start justify-between gap-3 rounded-xl bg-green-500/10 p-3">
              <div className="flex items-start gap-2 min-w-0">
                <FileArchive className="size-4 shrink-0 text-green-600 dark:text-green-400 mt-0.5" />
                <div className="min-w-0">
                  <p className="text-xs font-medium text-green-700 dark:text-green-400 truncate">
                    {zipName}
                  </p>
                  <p className="text-[11px] text-green-700/70 dark:text-green-400/70 mt-0.5">
                    格式：{format === "modrinth" ? "Modrinth" : "CurseForge"}
                  </p>
                </div>
              </div>
              <Button size="sm" className="shrink-0" onClick={startInstall}>
                开始安装
              </Button>
            </div>
          )}

          {status === "error" && (
            <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3">
              <AlertCircle className="size-4 shrink-0 text-destructive mt-0.5" />
              <p className="text-xs text-destructive break-all">{errorMsg}</p>
            </div>
          )}

          {/* 安装进度 */}
          {status === "installing" && (
            <div className="space-y-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{stage}</span>
                <span className="font-medium tabular-nums">{percentText}</span>
              </div>

              {/* 进度条 */}
              <div className="h-2 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full bg-primary transition-all duration-300 ease-out"
                  style={{
                    width: `${Math.min(100, Math.max(0, percent))}%`,
                  }}
                />
              </div>

              {/* 当前文件 */}
              {currentFile && (
                <div className="flex items-start gap-2 text-[11px] text-muted-foreground font-mono truncate">
                  <Package className="size-3 shrink-0 mt-0.5" />
                  <span className="truncate">{currentFile}</span>
                </div>
              )}

              {/* 文件计数 */}
              {totalFiles > 0 && (
                <div className="flex items-center justify-between text-[11px] text-muted-foreground">
                  <span>
                    已完成 {downloadedFiles} / {totalFiles} 个文件
                  </span>
                </div>
              )}
            </div>
          )}

          {/* 安装完成详情 */}
          {status === "success" && result && (
            <div className="rounded-xl border border-border bg-muted/30 p-4 space-y-2">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="text-sm font-medium text-foreground">
                    ✅ {result.message || "整合包安装完成"}
                  </p>
                  {result.instance_name && (
                    <p className="text-xs text-muted-foreground mt-1 font-mono">
                      实例名：{result.instance_name}
                    </p>
                  )}
                  {result.file_count !== undefined && result.file_count !== null && (
                    <p className="text-xs text-muted-foreground mt-0.5 font-mono">
                      外部资源：{result.file_count} 个文件
                    </p>
                  )}
                </div>
              </div>
              <p className="text-[11px] text-muted-foreground leading-relaxed pt-1 border-t border-border/60">
                安装完成后，你可以在「启动版本」中选择该整合包实例进行启动。
              </p>
            </div>
          )}
        </div>

        {/* 底部操作栏 */}
        <div className="flex items-center justify-end gap-2 px-6 py-4 border-t border-border bg-muted/20">
          {status === "installing" ? (
            <Button variant="destructive" size="sm" onClick={cancelInstall}>
              取消安装
            </Button>
          ) : status === "success" ? (
            <Button variant="default" size="sm" onClick={onClose}>
              完成
            </Button>
          ) : (
            <Button variant="ghost" size="sm" onClick={onClose}>
              关闭
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

// --------------------------------------------------------------------------
// 一个 Hook 形式的辅助组件：让父页面能方便地显示/隐藏对话框
// --------------------------------------------------------------------------

export function useModpackInstaller() {
  const [open, setOpen] = useState(false);
  return {
    open,
    show: () => setOpen(true),
    hide: () => setOpen(false),
  };
}