"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { AlertCircle, CheckCircle2, Loader2, UploadCloud, X } from "lucide-react";

import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import { cn } from "@/lib/utils";

type InstallPhase = "waiting" | "installing" | "success" | "error";

export function OpenP2PInstaller() {
  const { installOpenP2P, status } = useMultiplayerContext();
  const router = useRouter();
  const [phase, setPhase] = useState<InstallPhase>("waiting");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [closing, setClosing] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const successTimerRef = useRef<number | null>(null);
  const closeTimerRef = useRef<number | null>(null);

  const visible =
    !dismissed &&
    (status === "not_installed" || phase !== "waiting" || closing);

  const beginClose = useCallback((afterClose?: () => void) => {
    setClosing(true);
    closeTimerRef.current = window.setTimeout(() => {
      setClosing(false);
      setPhase("waiting");
      afterClose?.();
    }, 300);
  }, []);

  const handleExit = () => {
    beginClose(() => {
      setDismissed(true);
      router.push("/");
    });
  };

  useEffect(() => {
    if (!visible) return;

    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      try {
        const { getCurrentWebviewWindow } = await import(
          "@tauri-apps/api/webviewWindow"
        );
        const webview = getCurrentWebviewWindow();
        const stopListening = await webview.onDragDropEvent(async (event) => {
          if (!visible) return;

          if (event.payload.type === "over") {
            setIsDragOver(true);
            return;
          }
          if (event.payload.type === "leave") {
            setIsDragOver(false);
            return;
          }
          if (event.payload.type !== "drop") return;

          setIsDragOver(false);
          const sourcePath = event.payload.paths?.[0];
          if (!sourcePath) return;

          setPhase("installing");
          setErrorMsg(null);
          try {
            await installOpenP2P(sourcePath);
            setPhase("success");
            successTimerRef.current = window.setTimeout(() => beginClose(), 500);
          } catch (error) {
            setPhase("error");
            setErrorMsg(
              typeof error === "string"
                ? error
                : error instanceof Error && error.message
                  ? error.message
                  : "安装失败"
            );
          }
        });

        if (cancelled) stopListening();
        else unlisten = stopListening;
      } catch {
        // Browser builds do not expose Tauri drag events.
      }
    };

    void setup();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [beginClose, installOpenP2P, visible]);

  useEffect(
    () => () => {
      if (successTimerRef.current !== null) window.clearTimeout(successTimerRef.current);
      if (closeTimerRef.current !== null) window.clearTimeout(closeTimerRef.current);
    },
    []
  );

  if (!visible) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className={cn(
          "absolute inset-0 bg-black/60 backdrop-blur-sm transition-opacity duration-300",
          closing ? "opacity-0" : "opacity-100"
        )}
      />
      <div
        className={cn(
          "relative z-10 mx-4 w-full max-w-md space-y-4 rounded-2xl border bg-background p-6 shadow-2xl transition-all duration-300",
          closing ? "scale-95 opacity-0" : "scale-100 opacity-100"
        )}
      >
        <button
          type="button"
          aria-label="关闭安装窗口"
          className="absolute right-3 top-3 flex size-8 items-center justify-center rounded-lg text-muted-foreground transition-colors hover:bg-muted/50 hover:text-foreground"
          onClick={handleExit}
          disabled={phase === "installing"}
        >
          <X className="size-4" />
        </button>

        <div className="space-y-1 pr-8">
          <h3 className="text-base font-semibold">安装 OpenP2P</h3>
          <p className="text-xs leading-relaxed text-muted-foreground">
            多人联机功能需要 openp2p 作为联机工具。请将 openp2p 可执行文件拖入此窗口完成安装。
          </p>
        </div>

        <div
          className={cn(
            "flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed px-4 py-10 text-center transition-colors duration-200",
            isDragOver
              ? "border-primary bg-primary/5 text-primary"
              : phase === "success"
                ? "border-green-500 bg-green-500/5"
                : phase === "error"
                  ? "border-destructive bg-destructive/5"
                  : "border-border text-muted-foreground",
            (phase === "installing" || phase === "success") && "pointer-events-none"
          )}
        >
          {phase === "installing" ? (
            <>
              <Loader2 className="size-6 animate-spin text-primary" />
              <p className="text-sm font-medium">安装中...</p>
            </>
          ) : phase === "success" ? (
            <>
              <CheckCircle2 className="size-6 text-green-500" />
              <p className="text-sm font-medium text-green-600 dark:text-green-400">安装成功</p>
            </>
          ) : (
            <>
              <UploadCloud className={cn("size-8", isDragOver && "text-primary")} />
              <div className="space-y-0.5">
                <p className="text-sm font-medium text-foreground">将 openp2p 可执行文件拖到此处</p>
                <p className="text-xs text-muted-foreground">
                  Windows 通常为 openp2p.exe，Linux 与 macOS 通常为 openp2p
                </p>
              </div>
            </>
          )}
        </div>

        {phase === "error" && errorMsg ? (
          <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
            <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
            <span className="break-all">{errorMsg}</span>
          </div>
        ) : null}
      </div>
    </div>
  );
}
