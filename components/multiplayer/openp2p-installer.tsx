"use client";

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Download,
  CheckCircle2,
  AlertCircle,
  Loader2,
  UploadCloud,
} from "lucide-react";
import { cn } from "@/lib/utils";

type InstallStatus = "checking" | "not_installed" | "installed" | "installing" | "error";

export function OpenP2PInstaller() {
  const { installOpenp2p } = useMultiplayerContext();

  const [status, setStatus] = useState<InstallStatus>("checking");
  const [installedPath, setInstalledPath] = useState<string | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [isDragOver, setIsDragOver] = useState(false);

  // 检查是否已安装
  useEffect(() => {
    invoke<boolean>("mp_check_openp2p").then((installed) => {
      setStatus(installed ? "installed" : "not_installed");
    });
  }, []);

  // 监听 Tauri 拖拽事件（可获取真实文件路径）
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      try {
        const { getCurrentWebviewWindow } = await import(
          "@tauri-apps/api/webviewWindow"
        );
        const webview = getCurrentWebviewWindow();

        const fn = await webview.onDragDropEvent(async (event) => {
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
            // 粗略判断是否为可执行文件（不强制，用户可能拖任意文件）
            setStatus("installing");
            setErrorMsg(null);
            try {
              const dest = await installOpenp2p(src);
              setInstalledPath(dest);
              setStatus("installed");
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

  return (
    <Card
      size="sm"
      className={cn(
        "transition-colors duration-200",
        isDragOver && "border-primary bg-primary/5"
      )}
    >
      <CardContent className="space-y-3">
        {/* 标题行 */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Download className="size-4 text-muted-foreground" />
            <span className="text-sm font-semibold">openp2p 程序</span>
          </div>
          <StatusBadge status={status} />
        </div>

        {/* 拖放区域 */}
        <div
          className={cn(
            "flex flex-col items-center justify-center gap-2 rounded-xl border-2 border-dashed px-4 py-6 text-center transition-colors duration-200",
            isDragOver
              ? "border-primary bg-primary/5 text-primary"
              : "border-border text-muted-foreground"
          )}
        >
          {status === "installing" ? (
            <>
              <Loader2 className="size-6 animate-spin text-primary" />
              <p className="text-xs">安装中...</p>
            </>
          ) : (
            <>
              <UploadCloud className={cn("size-6", isDragOver && "text-primary")} />
              <p className="text-xs font-medium">
                {status === "installed" ? "拖入新版本以更新" : "将 openp2p 可执行文件拖到此处"}
              </p>
              <p className="text-[11px]">
                {status === "installed"
                  ? "当前已安装，可拖入新版本覆盖安装"
                  : "首次使用需先安装 openp2p，从 openp2p GitHub Releases 下载对应平台的版本"}
              </p>
            </>
          )}
        </div>

        {/* 已安装路径 */}
        {status === "installed" && installedPath && (
          <p className="text-[11px] text-muted-foreground break-all">
            已安装至：<code className="font-mono">{installedPath}</code>
          </p>
        )}

        {/* 错误信息 */}
        {status === "error" && errorMsg && (
          <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
            <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
            <span>{errorMsg}</span>
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function StatusBadge({ status }: { status: InstallStatus }) {
  if (status === "checking") {
    return (
      <Badge variant="secondary" className="text-[10px] gap-1">
        <Loader2 className="size-2.5 animate-spin" />
        检测中
      </Badge>
    );
  }
  if (status === "installed") {
    return (
      <Badge className="text-[10px] gap-1 bg-green-500/15 text-green-600 dark:text-green-400 border-green-500/20">
        <CheckCircle2 className="size-2.5" />
        已安装
      </Badge>
    );
  }
  if (status === "installing") {
    return (
      <Badge variant="secondary" className="text-[10px] gap-1">
        <Loader2 className="size-2.5 animate-spin" />
        安装中
      </Badge>
    );
  }
  if (status === "error") {
    return (
      <Badge variant="destructive" className="text-[10px] gap-1">
        <AlertCircle className="size-2.5" />
        安装失败
      </Badge>
    );
  }
  return (
    <Badge variant="outline" className="text-[10px] text-muted-foreground">
      未安装
    </Badge>
  );
}
