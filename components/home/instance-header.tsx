"use client";

import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import type { Account } from "@/types";
import { cn } from "@/lib/utils";
import { Play, Settings, Loader2 } from "lucide-react";
import { useLaunchContext } from "@/components/launch/launch-provider";
import Link from "next/link";

interface InstanceHeaderProps {
  instanceName: string;
  minecraftVersion: string;
  loader: string;
  modsCount: number;
  selectedProfile: Account | null;
  onOpenProfileSelector: () => void;
  className?: string;
}

export function InstanceHeader({
  instanceName,
  minecraftVersion,
  loader,
  modsCount,
  selectedProfile,
  onOpenProfileSelector,
  className,
}: InstanceHeaderProps) {
  return (
    <Card
      className={cn(
        "flex flex-row items-center justify-between gap-4",
        className
      )}
    >
      <div className="flex items-center gap-4 px-4">
        {/* 实例图标 */}
        <div className="w-12 h-12 rounded-xl bg-gradient-to-r from-blue-500 to-cyan-500 flex items-center justify-center">
          <span className="text-white font-bold text-lg">
            {instanceName.charAt(0)}
          </span>
        </div>
        {/* 实例信息 */}
        <div>
          <h2 className="font-bold text-base">{instanceName}</h2>
          <div className="flex items-center gap-2 mt-0.5">
            <Badge variant="secondary" className="text-xs">
              {minecraftVersion}
            </Badge>
            <Badge variant="outline" className="text-xs">
              {loader}
            </Badge>
            <span className="text-xs text-muted-foreground">
              {modsCount} mods
            </span>
          </div>
        </div>
      </div>

      <div className="flex items-center gap-2 px-4">
        {/* 账户选择 */}
        <button
          type="button"
          className="flex items-center gap-2 rounded-xl p-2 transition-colors hover:bg-accent"
          onClick={onOpenProfileSelector}
        >
          <Avatar size="sm">
            <AvatarFallback>
              {(selectedProfile?.name ?? "U").charAt(0).toUpperCase()}
            </AvatarFallback>
          </Avatar>
          <span className="text-sm font-medium">
            {selectedProfile?.name ?? "选择账户"}
          </span>
        </button>

        <Button variant="ghost" size="icon" asChild>
          <Link href="/launch">
            <Settings className="size-4" />
          </Link>
        </Button>

        <LaunchButton />
      </div>
    </Card>
  );
}

function LaunchButton() {
  const { status, launchGame } = useLaunchContext();
  const isLaunching = status === "preparing" || status === "launching";
  const isRunning = status === "running";
  const canLaunch = !isLaunching && !isRunning;

  return (
    <Button
      size="default"
      className="gap-1.5"
      disabled={!canLaunch}
      onClick={() => launchGame()}
    >
      {isLaunching ? (
        <>
          <Loader2 className="size-4 animate-spin" />
          启动中
        </>
      ) : isRunning ? (
        <>
          <Play className="size-4" />
          运行中
        </>
      ) : (
        <>
          <Play className="size-4" />
          启动
        </>
      )}
    </Button>
  );
}
