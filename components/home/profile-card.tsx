"use client";

import { motion } from "framer-motion";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { fadeSlideUp } from "@/lib/motion";
import { Loader2, Play, Users, UserPlus, Square } from "lucide-react";
import type { Account } from "@/types";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { VersionSelectorDialog } from "@/components/launch/version-selector-dialog";
import { AppUpdateSection } from "@/components/settings/app-updater";

type ProfileCardProps = {
  selectedProfile: Account | null;
  onOpenProfileSelector: () => void;
};

export function ProfileCard({
  selectedProfile,
  onOpenProfileSelector,
}: ProfileCardProps) {
  const { config, status, launchGame, cancelLaunch, errorMessage } = useLaunchContext();
  const isLaunching = status === "preparing" || status === "launching";
  const isRunning = status === "running";
  const canLaunch = !isLaunching && !isRunning;

  const versionDisplay = config.versionName || "未选择游戏版本";

  const handleLaunch = () => {
    if (isLaunching || isRunning) {
      cancelLaunch();
    } else {
      launchGame();
    }
  };

  return (
    <motion.div
      variants={fadeSlideUp}
      initial="initial"
      animate="animate"
      transition={{ delay: 0.1 }}
      className="h-full"
    >
      <Card className="h-full flex flex-col border shadow-sm">
        <CardHeader className="pb-3">
          <CardTitle className="text-base">账户与启动</CardTitle>
        </CardHeader>

        {/* 当前账户状态 */}
        <CardContent className="space-y-3 pt-0 pb-3">
          <div className="rounded-lg border border-border bg-card p-3">
            <div className="flex items-center gap-3">
              <div className="size-10 shrink-0 rounded-lg bg-muted flex items-center justify-center text-base font-semibold text-muted-foreground">
                {selectedProfile ? (
                  selectedProfile.name.charAt(0).toUpperCase()
                ) : (
                  <UserPlus className="size-5" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <div className="text-sm font-medium truncate">
                  {selectedProfile?.name ?? "尚未登录"}
                </div>
                <div className="text-xs text-muted-foreground truncate">
                  {selectedProfile ? "已登录" : "点击下方按钮添加账户"}
                </div>
              </div>
            </div>

            <div className="mt-3">
              <Button
                variant="outline"
                size="sm"
                onClick={onOpenProfileSelector}
                className="w-full gap-2"
              >
                <Users className="size-3.5" />
                管理账户
              </Button>
            </div>
          </div>

          {/* 版本选择 + 启动按钮 */}
          <div className="rounded-lg border border-border bg-card p-3 space-y-2">
            <div className="text-xs text-muted-foreground">
              当前版本：<span className="font-medium text-foreground">{versionDisplay}</span>
            </div>
            <Button
              size="lg"
              className="w-full gap-2 text-sm font-semibold"
              onClick={handleLaunch}
              disabled={!canLaunch && !isLaunching && !isRunning}
            >
              {isLaunching ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  {status === "preparing" ? "停止准备" : "停止启动"}
                </>
              ) : isRunning ? (
                <>
                  <Square className="size-4" />
                  停止游戏
                </>
              ) : (
                <>
                  <Play className="size-4" />
                  启动游戏
                </>
              )}
            </Button>
            {errorMessage && (
              <p className="text-xs text-destructive">{errorMessage}</p>
            )}
            <VersionSelectorDialog />
          </div>

          {/* 更新检查 */}
          <AppUpdateSection />
        </CardContent>
      </Card>
    </motion.div>
  );
}