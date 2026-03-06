"use client";

import { motion } from "framer-motion";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { fadeSlideUp } from "@/lib/motion";
import { Loader2, Play } from "lucide-react";
import type { Account } from "@/types";
import { useLaunchContext } from "@/components/launch/launch-provider";
import Link from "next/link";

type ProfileCardProps = {
  selectedProfile: Account | null;
  onOpenProfileSelector: () => void;
  instanceName: string;
  versionDisplay: string;
};

export function ProfileCard({
  selectedProfile,
  onOpenProfileSelector,
  instanceName,
  versionDisplay,
}: ProfileCardProps) {
  const { status, launchGame, errorMessage } = useLaunchContext();
  const isLaunching = status === "preparing" || status === "launching";
  const isRunning = status === "running";
  const canLaunch = !isLaunching && !isRunning;

  const handleLaunch = () => {
    launchGame({ versionName: instanceName });
  };

  return (
    <motion.div
      variants={fadeSlideUp}
      initial="initial"
      animate="animate"
      transition={{ delay: 0.1 }}
      className="h-full"
    >
      <Card className="h-full flex flex-col justify-between border shadow-sm hover:shadow-xl transition-shadow duration-300">
        {/* 卡片主要内容区域 */}
        <CardContent className="flex-grow flex flex-col items-center justify-center">
          <button
            type="button"
            className="cursor-pointer transition-all duration-300 flex items-center gap-3 p-3 w-full rounded-xl hover:bg-accent"
            onClick={onOpenProfileSelector}
          >
            <Avatar>
              {selectedProfile?.skinUrl && (
                <AvatarImage src={selectedProfile.skinUrl} alt={selectedProfile.name} />
              )}
              <AvatarFallback>
                {(selectedProfile?.name ?? "RTL User").charAt(0).toUpperCase()}
              </AvatarFallback>
            </Avatar>
            <div className="flex flex-col text-left">
              <span className="font-bold text-base">
                {selectedProfile?.name ?? "RTL User"}
              </span>
              <span className="text-muted-foreground text-xs">
                {selectedProfile?.status ?? ""}
              </span>
            </div>
          </button>
        </CardContent>

        {/* 卡片底部按钮区域 */}
        <CardContent className="flex items-center">
          <div className="w-full">
            <div className="text-center text-sm text-muted-foreground mb-2">
              {versionDisplay || "未选择实例"}
            </div>
            <Button
              variant="default"
              size="lg"
              className="w-full mb-2 gap-1.5"
              disabled={!canLaunch}
              onClick={handleLaunch}
            >
              {isLaunching ? (
                <><Loader2 className="size-4 animate-spin" />启动中</>
              ) : isRunning ? (
                <><Play className="size-4" />运行中</>
              ) : (
                <><Play className="size-4" />启动游戏</>
              )}
            </Button>
            {errorMessage && (
              <p className="text-xs text-destructive text-center mb-2">{errorMessage}</p>
            )}
            <div className="flex gap-2 mt-2">
              <Button variant="secondary" className="flex-1" asChild>
                <Link href="/launch">版本管理</Link>
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}
