"use client";

import { useEffect, useRef, useState } from "react";
import { Check, Copy, Loader2, Power, ScrollText, Server, Users } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { RunMode } from "@/components/multiplayer/multiplayer-state";

type MultiplayerRunningPanelProps = {
  runMode: RunMode | null;
  roomInfo: string | null;
  logText: string;
  openP2PDirectory: string;
  onStop: () => void | Promise<void>;
  onClearLog: () => void;
};

export function MultiplayerRunningPanel({
  runMode,
  roomInfo,
  logText,
  openP2PDirectory,
  onStop,
  onClearLog,
}: MultiplayerRunningPanelProps) {
  const [autoScroll, setAutoScroll] = useState(true);
  const [copied, setCopied] = useState(false);
  const logRef = useRef<HTMLDivElement>(null);
  const copyTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (autoScroll && logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [autoScroll, logText]);

  useEffect(
    () => () => {
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
    },
    []
  );

  const copyRoomInfo = async () => {
    if (!roomInfo) return;
    try {
      await navigator.clipboard.writeText(roomInfo);
      setCopied(true);
      if (copyTimerRef.current !== null) window.clearTimeout(copyTimerRef.current);
      copyTimerRef.current = window.setTimeout(() => setCopied(false), 2_000);
    } catch (error) {
      console.error("复制失败:", error);
    }
  };

  const runningModeLabel =
    runMode === "host" ? "房主模式" : runMode === "join" ? "加入模式" : "后台进程";

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border border-border bg-card">
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-border bg-muted/30 px-4 py-3">
        <div className="flex items-center gap-2 text-sm text-foreground">
          <div className="size-2 animate-pulse rounded-full bg-green-500" />
          <span className="font-medium">OpenP2P 正在运行（{runningModeLabel}）</span>
        </div>
        <Button variant="destructive" size="sm" onClick={() => void onStop()} className="gap-1.5">
          <Power className="size-3.5" />
          <span>停止联机</span>
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden lg:flex-row">
        <div className="flex flex-col items-center justify-center gap-5 overflow-auto border-border p-6 lg:w-[45%] lg:border-r">
          <div className="flex size-20 shrink-0 items-center justify-center rounded-2xl bg-green-500/10">
            {runMode === "host" ? (
              <Server className="size-10 text-green-600 dark:text-green-400" />
            ) : (
              <Users className="size-10 text-green-600 dark:text-green-400" />
            )}
          </div>

          <div className="max-w-md space-y-2 text-center">
            <h2 className="text-lg font-semibold text-foreground">
              {runMode === "host"
                ? "房间已创建"
                : runMode === "join"
                  ? "已加入房间"
                  : "OpenP2P 已在后台运行"}
            </h2>
            <p className="text-sm leading-relaxed text-muted-foreground">
              OpenP2P 正在后台静默运行，不会弹出任何命令窗口。
              下方日志面板实时显示运行状态反馈。
            </p>
          </div>

          {runMode === "host" && roomInfo ? (
            <div className="w-full max-w-lg space-y-2">
              <div className="text-center text-xs text-muted-foreground">
                将此编码分享给其他玩家以加入你的房间：
              </div>
              <div className="flex items-center gap-2 rounded-lg border border-border bg-background/50 px-3 py-2">
                <code className="flex-1 break-all font-mono text-xs text-foreground">{roomInfo}</code>
                <Button variant="outline" size="sm" onClick={copyRoomInfo} className="shrink-0 gap-1.5">
                  {copied ? <Check className="size-3.5 text-green-600" /> : <Copy className="size-3.5" />}
                  <span>{copied ? "已复制" : "复制"}</span>
                </Button>
              </div>
            </div>
          ) : null}

          {runMode === "join" && roomInfo ? (
            <div className="w-full max-w-lg space-y-2">
              <div className="text-center text-xs text-muted-foreground">当前使用的房间编码：</div>
              <div className="rounded-lg border border-border bg-background/50 px-3 py-2">
                <code className="break-all font-mono text-xs text-muted-foreground">{roomInfo}</code>
              </div>
            </div>
          ) : null}
        </div>

        <div className="flex min-h-0 flex-1 flex-col border-t border-border lg:border-t-0">
          <div className="flex shrink-0 items-center justify-between bg-muted/20 px-4 py-2">
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <ScrollText className="size-3.5" />
              <span className="font-medium">运行日志</span>
              {openP2PDirectory ? (
                <span className="text-[10px] text-muted-foreground/70">（来自 {openP2PDirectory}）</span>
              ) : null}
            </div>
            <div className="flex items-center gap-2">
              <label className="flex cursor-pointer items-center gap-1.5 text-[11px] text-muted-foreground">
                <input
                  type="checkbox"
                  checked={autoScroll}
                  onChange={(event) => setAutoScroll(event.target.checked)}
                  className="rounded border-border"
                />
                自动滚动
              </label>
              <Button variant="ghost" size="sm" onClick={onClearLog} className="h-6 px-2 text-[10px]">
                清空
              </Button>
            </div>
          </div>
          <div
            ref={logRef}
            className="min-h-0 flex-1 overflow-auto bg-[#071a2e] p-3 font-mono text-[11px] leading-relaxed dark:bg-[#061322]"
          >
            {logText ? (
              <pre className="whitespace-pre-wrap break-all text-green-400/90 dark:text-green-300/90">
                {logText}
              </pre>
            ) : (
              <div className="flex h-full flex-col items-center justify-center gap-2 text-slate-400">
                <Loader2 className="size-4 animate-spin" />
                <p className="text-[11px]">正在等待 OpenP2P 输出日志...</p>
                <p className="text-[10px]">如果长时间没有内容，可能是 openp2p 启动失败或参数不正确</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
