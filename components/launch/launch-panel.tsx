"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { useAccountContext } from "@/components/accounts/account-provider";
import { LaunchStatusBadge } from "@/components/launch/launch-status-badge";
import {
  Play,
  Loader2,
  AlertCircle,
  Gamepad2,
} from "lucide-react";

/**
 * 启动操作面板
 * 显示启动按钮和当前状态概览
 */
export function LaunchPanel() {
  const { config, status, errorMessage, launchGame } = useLaunchContext();
  const { selectedProfile } = useAccountContext();

  const isLaunching = status === "preparing" || status === "launching";
  const isRunning = status === "running";
  const canLaunch = !isLaunching && !isRunning;

  return (
    <Card size="sm">
      <CardContent className="space-y-4">
        {/* 状态与版本概览 */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-xl bg-primary/10">
              <Gamepad2 className="size-5 text-primary" />
            </div>
            <div>
              <p className="text-sm font-medium">
                {config.versionName || "未选择版本"}
              </p>
              <div className="flex items-center gap-2 mt-0.5">
                {config.loadType !== "0" && config.loadName && (
                  <Badge variant="secondary" className="text-[10px]">
                    {config.loadName}
                  </Badge>
                )}
                <LaunchStatusBadge status={status} />
              </div>
            </div>
          </div>

          {/* 当前账户 */}
          {selectedProfile && (
            <div className="flex items-center gap-2">
              <Avatar size="sm">
                <AvatarFallback>
                  {selectedProfile.name.charAt(0).toUpperCase()}
                </AvatarFallback>
              </Avatar>
              <div className="text-right">
                <p className="text-xs font-medium">{selectedProfile.name}</p>
                <p className="text-[10px] text-muted-foreground">
                  {selectedProfile.status}
                </p>
              </div>
            </div>
          )}
        </div>

        {/* 错误信息 */}
        {errorMessage && (
          <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
            <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
            <span>{errorMessage}</span>
          </div>
        )}

        {/* 启动按钮 */}
        <Button
          size="lg"
          className="w-full gap-2 text-sm font-semibold"
          onClick={() => launchGame()}
          disabled={!canLaunch}
        >
          {isLaunching ? (
            <>
              <Loader2 className="size-4 animate-spin" />
              {status === "preparing" ? "准备中..." : "启动中..."}
            </>
          ) : isRunning ? (
            <>
              <Play className="size-4" />
              游戏运行中
            </>
          ) : (
            <>
              <Play className="size-4" />
              启动游戏
            </>
          )}
        </Button>

        {/* 快捷信息 */}
        <div className="grid grid-cols-3 gap-2 text-[10px] text-muted-foreground">
          <div className="rounded-lg bg-muted/50 p-2">
            <span className="block font-medium text-foreground">内存</span>
            {config.maxMemory || "未设置"} MB
          </div>
          <div className="rounded-lg bg-muted/50 p-2">
            <span className="block font-medium text-foreground">Java</span>
            <span className="truncate block">
              {config.javaPath
                ? config.javaPath.split("/").pop() ||
                  config.javaPath.split("\\").pop()
                : "未设置"}
            </span>
          </div>
          <div className="rounded-lg bg-muted/50 p-2">
            <span className="block font-medium text-foreground">窗口</span>
            {config.windowWidth || "873"} × {config.windowHeight || "486"}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
