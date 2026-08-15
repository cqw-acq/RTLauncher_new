"use client";

import { useState } from "react";
import { AlertCircle, Power, Server, Users } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { RunMode } from "@/components/multiplayer/multiplayer-state";

type MultiplayerSetupPanelProps = {
  onStartHost: (roomName: string, port: string) => Promise<unknown>;
  onStartJoin: (encodedInfo: string, playerName: string) => Promise<unknown>;
};

export function MultiplayerSetupPanel({
  onStartHost,
  onStartJoin,
}: MultiplayerSetupPanelProps) {
  const [mode, setMode] = useState<RunMode>("host");
  const [roomName, setRoomName] = useState("");
  const [port, setPort] = useState("25565");
  const [encodedInfo, setEncodedInfo] = useState("");
  const [playerName, setPlayerName] = useState("");

  const handleStart = async () => {
    try {
      if (mode === "host") {
        const normalizedRoomName = roomName.trim();
        const normalizedPort = port.trim();
        if (!normalizedRoomName || !normalizedPort) return;
        await onStartHost(normalizedRoomName, normalizedPort);
        return;
      }

      const normalizedInfo = encodedInfo.trim();
      const normalizedPlayerName = playerName.trim();
      if (!normalizedInfo || !normalizedPlayerName) return;
      await onStartJoin(normalizedInfo, normalizedPlayerName);
    } catch (error) {
      console.error("启动失败:", error);
    }
  };

  return (
    <div className="flex flex-1 flex-col gap-6 rounded-xl border border-border bg-card p-6">
      <div className="flex items-center justify-center gap-2">
        <Button
          variant={mode === "host" ? "default" : "outline"}
          size="sm"
          onClick={() => setMode("host")}
          className="gap-1.5"
        >
          <Server className="size-3.5" />
          <span>创建房间（房主）</span>
        </Button>
        <Button
          variant={mode === "join" ? "default" : "outline"}
          size="sm"
          onClick={() => setMode("join")}
          className="gap-1.5"
        >
          <Users className="size-3.5" />
          <span>加入房间</span>
        </Button>
      </div>

      {mode === "host" ? (
        <div className="mx-auto flex w-full max-w-md flex-1 flex-col items-center justify-center gap-5">
          <div className="flex size-16 items-center justify-center rounded-2xl bg-primary/10">
            <Server className="size-8 text-primary" />
          </div>
          <div className="w-full space-y-1 text-center">
            <h2 className="text-base font-semibold text-foreground">创建房间</h2>
            <p className="text-xs leading-relaxed text-muted-foreground">
              设置房间名和游戏端口号。启动后将生成房间编码，分享给其他玩家即可加入。
            </p>
          </div>

          <div className="w-full space-y-3">
            <div className="space-y-1.5">
              <label htmlFor="multiplayer-room-name" className="text-xs font-medium text-foreground">
                房间名
              </label>
              <input
                id="multiplayer-room-name"
                type="text"
                value={roomName}
                onChange={(event) => setRoomName(event.target.value)}
                placeholder="例如：my_room"
                className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/50"
              />
            </div>

            <div className="space-y-1.5">
              <label htmlFor="multiplayer-port" className="text-xs font-medium text-foreground">
                端口号
              </label>
              <input
                id="multiplayer-port"
                type="text"
                value={port}
                onChange={(event) => setPort(event.target.value)}
                placeholder="例如：25565"
                className="w-full rounded-lg border border-border bg-background px-3 py-2 font-mono text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/50"
              />
              <p className="flex items-start gap-1.5 text-sm font-semibold leading-relaxed text-red-600 dark:text-red-400">
                <AlertCircle className="mt-0.5 size-4 shrink-0" />
                <span>重要：请填写 Minecraft“对局域网开放”后显示的局域网联机端口号！</span>
              </p>
            </div>

            <Button
              onClick={handleStart}
              className="mt-2 w-full gap-2"
              disabled={!roomName.trim() || !port.trim()}
            >
              <Power className="size-4" />
              <span>启动联机</span>
            </Button>
          </div>
        </div>
      ) : (
        <div className="mx-auto flex w-full max-w-md flex-1 flex-col items-center justify-center gap-5">
          <div className="flex size-16 items-center justify-center rounded-2xl bg-primary/10">
            <Users className="size-8 text-primary" />
          </div>
          <div className="w-full space-y-1 text-center">
            <h2 className="text-base font-semibold text-foreground">加入房间</h2>
            <p className="text-xs leading-relaxed text-muted-foreground">
              输入房主提供的房间编码和你的玩家名，即可加入房间。
            </p>
          </div>

          <div className="w-full space-y-3">
            <div className="space-y-1.5">
              <label htmlFor="multiplayer-room-code" className="text-xs font-medium text-foreground">
                房间编码
              </label>
              <input
                id="multiplayer-room-code"
                type="text"
                value={encodedInfo}
                onChange={(event) => setEncodedInfo(event.target.value)}
                placeholder="房主分享的 Base64 编码"
                className="w-full rounded-lg border border-border bg-background px-3 py-2 font-mono text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/50"
              />
            </div>

            <div className="space-y-1.5">
              <label htmlFor="multiplayer-player-name" className="text-xs font-medium text-foreground">
                玩家名
              </label>
              <input
                id="multiplayer-player-name"
                type="text"
                value={playerName}
                onChange={(event) => setPlayerName(event.target.value)}
                placeholder="你的游戏玩家名"
                className="w-full rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground outline-none focus:border-primary focus:ring-2 focus:ring-primary/50"
              />
            </div>

            <Button
              onClick={handleStart}
              className="mt-2 w-full gap-2"
              disabled={!encodedInfo.trim() || !playerName.trim()}
            >
              <Power className="size-4" />
              <span>加入联机</span>
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
