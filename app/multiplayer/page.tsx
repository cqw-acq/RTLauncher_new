"use client";

import { useEffect, useRef, useState } from "react";
import { AlertCircle, Check, Copy, Globe, Loader2, RefreshCw } from "lucide-react";

import { MultiplayerRunningPanel } from "@/components/multiplayer/multiplayer-running-panel";
import { MultiplayerSetupPanel } from "@/components/multiplayer/multiplayer-setup-panel";
import { OpenP2PInstaller } from "@/components/multiplayer/openp2p-installer";
import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import type { OpenP2PStatus } from "@/components/multiplayer/multiplayer-state";
import { useMultiplayerLogPolling } from "@/components/multiplayer/use-multiplayer-log-polling";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export default function MultiplayerPage() {
  const {
    status,
    errorMsg,
    runMode,
    roomInfo,
    logText,
    checkStatus,
    startAsHost,
    startAsJoin,
    stopOpenP2P,
    pollLog,
    clearLog,
    getOpenP2PPaths,
  } = useMultiplayerContext();
  const [openP2PDirectory, setOpenP2PDirectory] = useState("");
  const [openP2PPath, setOpenP2PPath] = useState("");
  const [pathCopied, setPathCopied] = useState(false);
  const copyTimerRef = useRef<number | null>(null);

  useEffect(() => {
    void checkStatus();
    void getOpenP2PPaths().then(({ directory, executable }) => {
      setOpenP2PDirectory(directory);
      setOpenP2PPath(executable);
    });
  }, [checkStatus, getOpenP2PPaths]);

  useEffect(
    () => () => {
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
    },
    []
  );

  useMultiplayerLogPolling(
    status === "running" || status === "starting",
    pollLog
  );

  const copyOpenP2PPath = async () => {
    if (!openP2PPath) return;
    try {
      await navigator.clipboard.writeText(openP2PPath);
      setPathCopied(true);
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = window.setTimeout(() => setPathCopied(false), 2_000);
    } catch (error) {
      console.error("复制失败:", error);
    }
  };

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden p-4">
      <header className="flex shrink-0 items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
            <Globe className="size-5 text-primary" />
          </div>
          <div>
            <h1 className="text-lg font-semibold leading-none">多人联机</h1>
            <p className="mt-1 text-xs text-muted-foreground">
              基于 OpenP2P 的联机工具 —— 后台运行，无命令窗口
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <StatusBadge status={status} />
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            onClick={() => void checkStatus()}
            title="刷新状态"
            aria-label="刷新状态"
          >
            <RefreshCw className="size-4" />
          </Button>
        </div>
      </header>

      {status === "error" && errorMsg ? (
        <div className="flex shrink-0 items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
          <AlertCircle className="mt-0.5 size-3.5 shrink-0" />
          <span className="break-all">{errorMsg}</span>
        </div>
      ) : null}

      {openP2PPath ? (
        <div className="flex shrink-0 items-center justify-between gap-2 rounded-lg bg-muted/30 px-3 py-2">
          <div className="min-w-0 flex-1">
            <span className="text-[10px] text-muted-foreground">OpenP2P 位置：</span>
            <code className="ml-1 truncate font-mono text-[11px] text-muted-foreground">
              {openP2PPath}
            </code>
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 shrink-0 gap-1 px-2 text-[10px]"
            onClick={copyOpenP2PPath}
          >
            {pathCopied ? <Check className="size-3" /> : <Copy className="size-3" />}
            <span>{pathCopied ? "已复制" : "复制"}</span>
          </Button>
        </div>
      ) : null}

      <main className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto">
        {status === "running" ? (
          <MultiplayerRunningPanel
            runMode={runMode}
            roomInfo={roomInfo}
            logText={logText}
            openP2PDirectory={openP2PDirectory}
            onStop={stopOpenP2P}
            onClearLog={clearLog}
          />
        ) : status === "starting" || status === "stopping" ? (
          <PendingPanel status={status} />
        ) : status === "installed" ? (
          <MultiplayerSetupPanel
            onStartHost={startAsHost}
            onStartJoin={startAsJoin}
          />
        ) : status === "not_installed" ? (
          <NotInstalledPanel />
        ) : (
          <CheckingPanel />
        )}
      </main>

      <OpenP2PInstaller />
    </div>
  );
}

function PendingPanel({ status }: { status: "starting" | "stopping" }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 rounded-xl border border-border bg-card text-muted-foreground">
      <Loader2 className="size-8 animate-spin text-primary" />
      <p className="text-sm font-medium text-foreground">
        {status === "starting" ? "正在启动 OpenP2P..." : "正在停止 OpenP2P..."}
      </p>
      <p className="text-xs text-muted-foreground">请稍候...</p>
    </div>
  );
}

function NotInstalledPanel() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4 rounded-xl border border-border bg-card p-6">
      <div className="flex size-16 items-center justify-center rounded-2xl bg-destructive/10">
        <AlertCircle className="size-8 text-destructive" />
      </div>
      <div className="space-y-1 text-center">
        <h2 className="text-base font-semibold text-foreground">尚未安装 OpenP2P</h2>
        <p className="max-w-md text-xs leading-relaxed text-muted-foreground">
          多人联机功能需要 openp2p.exe 作为外置联机工具。
          请将 openp2p 可执行文件拖入此窗口完成安装。
        </p>
      </div>
      <div className="mt-2 flex items-center gap-2 rounded-lg bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
        <div className="size-1.5 rounded-full bg-primary" />
        <span>将 openp2p.exe 文件拖入窗口即可完成安装</span>
      </div>
    </div>
  );
}

function CheckingPanel() {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 rounded-xl border border-border bg-card text-muted-foreground">
      <Loader2 className="size-6 animate-spin text-primary" />
      <p className="text-sm">正在检查 OpenP2P 状态...</p>
    </div>
  );
}

const STATUS_CONFIG: Record<
  OpenP2PStatus,
  { label: string; dot: string; background: string; text: string }
> = {
  idle: {
    label: "检查中",
    dot: "bg-muted-foreground/40",
    background: "bg-muted",
    text: "text-muted-foreground",
  },
  not_installed: {
    label: "未安装",
    dot: "bg-destructive",
    background: "bg-destructive/10",
    text: "text-destructive",
  },
  installed: {
    label: "就绪",
    dot: "bg-emerald-500",
    background: "bg-emerald-500/10",
    text: "text-emerald-700 dark:text-emerald-400",
  },
  starting: {
    label: "启动中",
    dot: "bg-primary",
    background: "bg-primary/10",
    text: "text-primary",
  },
  running: {
    label: "运行中",
    dot: "bg-green-500 animate-pulse",
    background: "bg-green-500/10",
    text: "text-green-700 dark:text-green-400",
  },
  stopping: {
    label: "停止中",
    dot: "bg-orange-500",
    background: "bg-orange-500/10",
    text: "text-orange-700 dark:text-orange-400",
  },
  error: {
    label: "出错",
    dot: "bg-destructive",
    background: "bg-destructive/10",
    text: "text-destructive",
  },
};

function StatusBadge({ status }: { status: OpenP2PStatus }) {
  const config = STATUS_CONFIG[status];
  return (
    <div
      className={cn(
        "flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium",
        config.background,
        config.text
      )}
    >
      <span className={cn("size-1.5 rounded-full", config.dot)} />
      {config.label}
    </div>
  );
}
