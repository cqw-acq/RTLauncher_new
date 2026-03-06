"use client";

import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import { fadeSlideUp } from "@/lib/motion";
import {
  Loader2,
  Server,
  Copy,
  Check,
  AlertCircle,
  Wifi,
  WifiOff,
} from "lucide-react";
import { cn } from "@/lib/utils";

export function HostPanel() {
  const {
    hostRoomName, setHostRoomName,
    hostPort, setHostPort,
    hostJoinCode,
    hostStatus, hostError,
    handleHostRoom, handleHostDisconnect,
  } = useMultiplayerContext();

  const [copied, setCopied] = useState(false);

  const isLoading = hostStatus === "loading";
  const isRunning = hostStatus === "running";

  const roomNameError = (() => {
    if (!hostRoomName) return null;
    if (/\s/.test(hostRoomName)) return "房间名称不能包含空格";
    if (/[^a-zA-Z0-9_\-]/.test(hostRoomName)) return "房间名称只能包含英文字母、数字、下划线和连字符";
    return null;
  })();

  const handleCopy = async () => {
    if (!hostJoinCode) return;
    await navigator.clipboard.writeText(hostJoinCode);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col gap-4">
      {/* 配置区 */}
      <Card size="sm">
        <CardContent className="space-y-4">
          <div className="flex items-center gap-3">
            <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
              <Server className="size-4 text-primary" />
            </div>
            <div>
              <p className="text-sm font-semibold">创建房间</p>
              <p className="text-xs text-muted-foreground">
                启动 openp2p 并生成联机码分享给好友
              </p>
            </div>
          </div>

          <div className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="host-room-name">房间名称</Label>
              <Input
                id="host-room-name"
                placeholder="你的节点名，建议用英文"
                value={hostRoomName}
                onChange={(e) => setHostRoomName(e.target.value)}
                disabled={isRunning || isLoading}
                className={cn(roomNameError && "border-destructive focus-visible:ring-destructive")}
              />
              {roomNameError && (
                <p className="flex items-center gap-1 text-[11px] text-destructive">
                  <AlertCircle className="size-3 shrink-0" />
                  {roomNameError}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="host-port">Minecraft 端口</Label>
              <Input
                id="host-port"
                placeholder="25565"
                value={hostPort}
                onChange={(e) => setHostPort(e.target.value)}
                disabled={isRunning || isLoading}
              />
              <p className="text-[11px] text-muted-foreground">
                与你的 Minecraft 服务器监听端口保持一致
              </p>
            </div>
          </div>

          {/* 错误提示 */}
          <AnimatePresence>
            {hostStatus === "error" && hostError && (
              <motion.div variants={fadeSlideUp} initial="initial" animate="animate" exit="exit">
                <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
                  <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
                  <span>{hostError}</span>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          {/* 运行状态 */}
          <AnimatePresence>
            {isRunning && (
              <motion.div variants={fadeSlideUp} initial="initial" animate="animate" exit="exit">
                <div className="flex items-center gap-2 rounded-xl bg-green-500/10 p-3 text-xs text-green-600 dark:text-green-400">
                  <Wifi className="size-3.5 shrink-0" />
                  <span>openp2p 已启动，等待好友连接...</span>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          <Button
            className="w-full gap-2"
            onClick={handleHostRoom}
            disabled={isLoading || isRunning || !hostRoomName.trim() || !hostPort.trim() || !!roomNameError}
          >
            {isLoading ? (
              <>
                <Loader2 className="size-4 animate-spin" />
                启动中...
              </>
            ) : isRunning ? (
              <>
                <Wifi className="size-4" />
                房间运行中
              </>
            ) : (
              <>
                <Server className="size-4" />
                创建房间
              </>
            )}
          </Button>

          <AnimatePresence>
            {isRunning && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                transition={{ duration: 0.2 }}
              >
                <Button
                  variant="outline"
                  className="w-full gap-2 text-destructive hover:text-destructive hover:bg-destructive/10"
                  onClick={handleHostDisconnect}
                >
                  <WifiOff className="size-4" />
                  断开连接
                </Button>
              </motion.div>
            )}
          </AnimatePresence>
        </CardContent>
      </Card>

      {/* 联机码 */}
      <AnimatePresence>
      {hostJoinCode && (
        <motion.div variants={fadeSlideUp} initial="initial" animate="animate" exit="exit">
        <Card size="sm">
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between">
              <p className="text-sm font-semibold">联机码</p>
              <Badge variant="secondary" className="text-[10px]">
                已生成
              </Badge>
            </div>
            <div className="flex items-center gap-2">
              <code
                className={cn(
                  "flex-1 rounded-lg bg-muted px-3 py-2.5 text-xs font-mono break-all",
                  "select-all cursor-text"
                )}
              >
                {hostJoinCode}
              </code>
              <Button
                variant="outline"
                size="icon"
                className="shrink-0 size-9"
                onClick={handleCopy}
                title="复制联机码"
              >
                {copied ? (
                  <Check className="size-3.5 text-green-500" />
                ) : (
                  <Copy className="size-3.5" />
                )}
              </Button>
            </div>
            <p className="text-[11px] text-muted-foreground">
              将联机码发给好友，好友在「加入房间」中填入即可连接
            </p>
          </CardContent>
        </Card>
        </motion.div>
      )}
      </AnimatePresence>
    </div>
  );
}
