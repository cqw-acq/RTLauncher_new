"use client";

import { useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { invoke } from "@tauri-apps/api/core";
import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import { AlertCircle, CheckCircle2, Loader2, UploadCloud, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

type InstallStatus = "checking" | "not_installed" | "installed" | "installing" | "success" | "error";

export function OpenP2PInstaller() {
  const { installOpenp2p } = useMultiplayerContext();
  const router = useRouter();

  const [status, setStatus] = useState<InstallStatus>("checking");
  const [installedPath, setInstalledPath] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [closing, setClosing] = useState(false);

  const handleExit = () => {
    setClosing(true);
    setTimeout(() => {
      setDialogOpen(false);
      setClosing(false);
      router.push("/");
    }, 300);
  };

  // 用 ref 追踪 dialogOpen，避免 effect 重新注册
  const dialogOpenRef = useRef(dialogOpen);
  dialogOpenRef.current = dialogOpen;

  // 检查是否已安装，未安装时自动弹窗
  useEffect(() => {
    invoke<boolean>("mp_check_openp2p").then((installed) => {
      if (installed) {
        setStatus("installed");
      } else {
        setStatus("not_installed");
        setDialogOpen(true);
      }
    });
  }, []);

  // 监听 Tauri 拖拽事件，仅在对话框打开时处理
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      try {
        const { getCurrentWebviewWindow } = await import(
          "@tauri-apps/api/webviewWindow"
        );
        const webview = getCurrentWebviewWindow();

        const fn = await webview.onDragDropEvent(async (event) => {
          if (!dialogOpenRef.current) return;

          if (event.payload.type === "over") {
            setIsDragOver(true);
            return;
          }
          if (event.payload.type === "leave") {
            setIsDragOver(false);
            return;
          }
          if (event.payload.type === "drop") {
            setIsDragOver(false);
            const paths = event.payload.paths;
            if (!paths || paths.length === 0) return;

            const src = paths[0];
            setStatus("installing");
            setErrorMsg(null);
            try {
              const dest = await installOpenp2p(src);
              setInstalledPath(dest);
              setStatus("success");
              setTimeout(() => {
                setClosing(true);
                setTimeout(() => {
                  setDialogOpen(false);
                  setClosing(false);
                  setStatus("installed");
                }, 300);
              }, 500);
            } catch (e) {
              setErrorMsg(typeof e === "string" ? e : (e as Error).message ?? "安装失败");
              setStatus("error");
            }
          }
        });

        unlisten = fn;
      } catch {
        // 非 Tauri 环境忽略
      }
    };

    setup();
    return () => {
      unlisten?.();
    };
  }, [installOpenp2p]);

  // 已安装则不渲染任何内容
  if (status === "installed" || status === "checking") return null;

  return (
    <>
      {/* 安装对话框 */}
      {dialogOpen && (
        <div
          className={cn(
            "fixed inset-0 z-50 flex items-center justify-center transition-opacity duration-300",
            closing ? "opacity-0" : "opacity-100"
          )}
        >          {/* 遮罩 */}
          <div className="absolute inset-0 bg-black/50 backdrop-blur-sm" />
          {/* 对话框 */}
          <div className="relative z-10 w-full max-w-md mx-4 rounded-2xl bg-background border shadow-2xl p-6 space-y-4">
            <Button
              variant="ghost"
              size="icon"
              className="absolute top-3 right-3 size-7 text-muted-foreground hover:text-foreground"
              onClick={handleExit}
              disabled={status === "installing"}
            >
              <X className="size-4" />
            </Button>
            <div className="space-y-0.5">
              <h3 className="text-sm font-semibold">安装 openp2p</h3>
              <p className="text-[11px] text-muted-foreground">
                联机功能需要 openp2p，请从{" "}
                <span className="font-mono text-foreground">openp2p GitHub Releases</span>{" "}
                下载对应平台的版本，将文件拖入此窗口完成安装
              </p>
            </div>

            {/* 拖放区域 */}
            <div
              className={cn(
                "flex flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed px-4 py-10 text-center transition-colors duration-200",
                isDragOver
                  ? "border-primary bg-primary/5 text-primary"
                  : status === "success"
                  ? "border-green-500 bg-green-500/5"
                  : "border-border text-muted-foreground",
                (status === "installing" || status === "success") && "pointer-events-none"
              )}
            >
              {status === "installing" ? (
                <>
                  <Loader2 className="size-6 animate-spin text-primary" />
                  <p className="text-xs">安装中...</p>
                </>
              ) : status === "success" ? (
                <>
                  <CheckCircle2 className="size-6 text-green-500" />
                  <p className="text-xs font-medium text-green-600 dark:text-green-400">安装成功</p>
                </>
              ) : (
                <>
                  <UploadCloud className={cn("size-7", isDragOver && "text-primary")} />
                  <p className="text-xs font-medium">将 openp2p 可执行文件拖到此处</p>
                  <p className="text-[11px]">
                    支持 Windows (openp2p.exe) 与 macOS / Linux (openp2p)
                  </p>
                </>
              )}
            </div>

            {status === "error" && errorMsg && (
              <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
                <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
                <span>{errorMsg}</span>
              </div>
            )}

          </div>
        </div>
      )}
    </>
  );
}
