"use client";

import { Globe } from "lucide-react";
import { useMultiplayerContext } from "@/components/multiplayer/multiplayer-provider";
import { HostPanel } from "@/components/multiplayer/host-panel";
import { JoinPanel } from "@/components/multiplayer/join-panel";
import { OpenP2PInstaller } from "@/components/multiplayer/openp2p-installer";
import { cn } from "@/lib/utils";

const TABS = [
  { id: "host" as const, label: "创建房间" },
  { id: "join" as const, label: "加入房间" },
];

export default function MultiplayerPage() {
  const { mode, setMode } = useMultiplayerContext();

  return (
    <div className="flex h-full flex-col gap-4 p-4 overflow-y-auto">
      {/* 页面标题 */}
      <div className="flex items-center gap-3 shrink-0">
        <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
          <Globe className="size-5 text-primary" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none">多人联机</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            基于 openp2p 的 P2P 联机，无需公网 IP
          </p>
        </div>
      </div>

      <div className="flex flex-col lg:flex-row gap-4 flex-1 min-h-0">
        {/* 左侧：模式面板 */}
        <div className="flex flex-col gap-4 w-full lg:max-w-md">
          {/* 模式切换 Tabs */}
          <div className="flex shrink-0 gap-1 rounded-xl bg-muted p-1 w-fit">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setMode(tab.id)}
                className={cn(
                  "rounded-lg px-4 py-1.5 text-sm font-medium transition-colors",
                  mode === tab.id
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                {tab.label}
              </button>
            ))}
          </div>

          {mode === "host" ? <HostPanel /> : <JoinPanel />}
        </div>

        {/* 右侧：openp2p 安装器（拖入文件安装） */}
        <div className="w-full lg:max-w-sm">
          <OpenP2PInstaller />
        </div>
      </div>
    </div>
  );
}
