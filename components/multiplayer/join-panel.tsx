"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import {
  Loader2,
  Users,
  AlertCircle,
  Wifi,
  WifiOff,
  ClipboardPaste,
} from "lucide-react";

export function JoinPanel() {
  const {
    joinCode, setJoinCode,
    joinPlayerName, setJoinPlayerName,
    joinStatus, joinError,
    handleJoinRoom, handleJoinDisconnect,
  } = useMultiplayerContext();

  const isLoading = joinStatus === "loading";
  const isRunning = joinStatus === "running";

  const handlePaste = async () => {
    try {
      const text = await navigator.clipboard.readText();
      setJoinCode(text.trim());
    } catch {
      // clipboard not accessible
    }
  };

  const handleJoin = () => handleJoinRoom();
  const handleDisconnect = () => handleJoinDisconnect();

  return (
    <Card size="sm">
      <CardContent className="space-y-4">
        <div className="flex items-center gap-3">
          <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
            <Users className="size-4 text-primary" />
          </div>
          <div>
            <p className="text-sm font-semibold">加入房间</p>
            <p className="text-xs text-muted-foreground">
              输入好友的联机码，连接到对方的 Minecraft 服务器
            </p>
          </div>
        </div>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="join-code">联机码</Label>
            <div className="flex gap-2">
              <Input
                id="join-code"
                placeholder="粘贴好友发来的联机码"
                value={joinCode}
                onChange={(e) => setJoinCode(e.target.value)}
                disabled={isRunning || isLoading}
                className="font-mono text-xs"
              />
              <Button
                variant="outline"
                size="icon"
                className="shrink-0 size-9"
                onClick={handlePaste}
                title="从剪贴板粘贴"
                disabled={isRunning || isLoading}
              >
                <ClipboardPaste className="size-3.5" />
              </Button>
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="join-player-name">玩家名</Label>
            <Input
              id="join-player-name"
              placeholder="你的节点名称（英文）"
              value={joinPlayerName}
              onChange={(e) => setJoinPlayerName(e.target.value)}
              disabled={isRunning || isLoading}
            />
            <p className="text-[11px] text-muted-foreground">
              openp2p 节点名，不需要与游戏 ID 相同
            </p>
          </div>
        </div>

        {/* 错误提示 */}
        {joinStatus === "error" && joinError && (
          <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
            <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
            <span>{joinError}</span>
          </div>
        )}

        {/* 运行状态 */}
        {isRunning && (
          <div className="flex items-center gap-2 rounded-xl bg-green-500/10 p-3 text-xs text-green-600 dark:text-green-400">
            <Wifi className="size-3.5 shrink-0" />
            <span>
              已成功连接！在 Minecraft 中以{" "}
              <code className="font-mono">127.0.0.1</code> 加入服务器即可
            </span>
          </div>
        )}

        <Button
          className="w-full gap-2"
          onClick={handleJoin}
          disabled={
            isLoading || isRunning || !joinCode.trim() || !joinPlayerName.trim()
          }
        >
          {isLoading ? (
            <>
              <Loader2 className="size-4 animate-spin" />
              连接中...
            </>
          ) : isRunning ? (
            <>
              <Wifi className="size-4" />
              已连接
            </>
          ) : (
            <>
              <Users className="size-4" />
              加入房间
            </>
          )}
        </Button>

        {isRunning && (
          <Button
            variant="outline"
            className="w-full gap-2 text-destructive hover:text-destructive hover:bg-destructive/10"
            onClick={handleDisconnect}
          >
            <WifiOff className="size-4" />
            断开连接
          </Button>
        )}
      </CardContent>
    </Card>
  );
}
