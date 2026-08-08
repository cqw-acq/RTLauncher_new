"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { useAccountContext } from "@/components/accounts/account-provider";
import { LaunchStatusBadge } from "@/components/launch/launch-status-badge";
import { LaunchProgress } from "@/components/launch/launch-progress";
import { LaunchProgressStages } from "@/components/launch/launch-progress-stages";
import { LaunchReportButton } from "@/components/launch/launch-report-button";
import { fadeSlideUp } from "@/lib/motion";
import { cn, getAvatarColor, getAvatarInitials } from "@/lib/utils";
import {
  Play,
  Loader2,
  AlertCircle,
  HardDrive,
  Sparkles,
  Square,
} from "lucide-react";
import { useState, useEffect } from "react";
import type { LauncherPathsConfig } from "@/types";
import { useI18n } from "@/components/i18n/use-i18n";
import { LoaderIcon, inferLoaderKind } from "@/components/launch/loader-icon";

interface MemoryOptimizationReport {
  available_before_mb: number;
  available_after_mb: number;
  freed_mb: number;
  total_mb: number;
  platform: string;
  methods: string[];
  duration_ms: number;
}

/**
 * 启动操作面板
 * 显示启动按钮和当前状态概览
 */
export function LaunchPanel() {
  const { t } = useI18n();
  const { config, status, errorMessage, launchGame, cancelLaunch } = useLaunchContext();
  const { selectedProfile } = useAccountContext();
  const [javaInstallations, setJavaInstallations] = useState<LauncherPathsConfig["java_installations"]>({});

  // 内存清理相关的状态
  const [optimizing, setOptimizing] = useState(false);
  const [lastReport, setLastReport] = useState<MemoryOptimizationReport | null>(null);
  const [optimizeError, setOptimizeError] = useState<string | null>(null);
  const profileStatusMap: Record<string, string> = {
    "LittleSkin 登录": t("account.littleSkinSignIn"),
    "第三方登录": t("account.thirdPartySignIn"),
    "离线登录": t("account.offlineSignIn"),
    "正版登录": t("account.microsoftSignIn"),
  };
  const profileStatus = selectedProfile?.status
    ? profileStatusMap[selectedProfile.status] ?? selectedProfile.status
    : undefined;

  useEffect(() => {
    invoke<LauncherPathsConfig>("get_launcher_paths_config")
      .then((cfg) => setJavaInstallations(cfg.java_installations ?? {}))
      .catch(() => {});
  }, []);

  const isLaunching = status === "preparing" || status === "launching";
  const isRunning = status === "running";
  const canLaunch = !isLaunching && !isRunning;

  async function handleOptimizeMemory() {
    if (optimizing) return;
    setOptimizing(true);
    setOptimizeError(null);
    try {
      const report = await invoke<MemoryOptimizationReport>("optimize_memory_usage");
      setLastReport(report);
      // 3 秒后恢复正常按钮状态（保留数据）
      setTimeout(() => {
        setOptimizing(false);
      }, 2500);
    } catch (err) {
      setOptimizeError(String(err));
      setOptimizing(false);
    }
  }

  return (
    <Card size="sm">
      <CardContent className="space-y-4">
        {/* 状态与版本概览 */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="flex size-10 items-center justify-center rounded-xl bg-primary/10 p-1.5">
              <LoaderIcon
                kind={inferLoaderKind(config.loadType === "0" ? "vanilla" : config.loadName)}
                className="size-full"
              />
            </div>
            <div>
              <p className="text-sm font-medium">
                {config.versionName || t("launch.noVersionSelected")}
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
                <AvatarFallback
                  className={cn(
                    getAvatarColor(selectedProfile.name),
                    "text-white font-medium"
                  )}
                >
                  {getAvatarInitials(selectedProfile.name)}
                </AvatarFallback>
              </Avatar>
              <div className="text-right">
                <p className="text-xs font-medium">{selectedProfile.name}</p>
                <p className="text-[10px] text-muted-foreground">
                  {profileStatus}
                </p>
              </div>
            </div>
          )}
        </div>

        {/* 错误信息 */}
        <AnimatePresence>
          {errorMessage && (
            <motion.div
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-xs text-destructive">
                <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
                <span>{errorMessage}</span>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* 启动进度 */}
        <LaunchProgress />

        {/* 详细启动阶段 */}
        <LaunchProgressStages />

        {/* 启动按钮 */}
        <Button
          size="lg"
          className="w-full gap-2 text-sm font-semibold"
          onClick={isLaunching || isRunning ? () => cancelLaunch() : () => launchGame()}
          disabled={!canLaunch && !isLaunching && !isRunning}
        >
          {isLaunching ? (
            <>
              <Loader2 className="size-4 animate-spin" />
              {status === "preparing" ? t("launch.panel.stopPreparing") : "Launching"}
            </>
          ) : isRunning ? (
            <>
              <Square className="size-4" />
              {t("launch.stopGame")}
            </>
          ) : (
            <>
              <Play className="size-4" />
              {t("launch.launchGame")}
            </>
          )}
        </Button>

        {/* 内存清理按钮 */}
        <Button
          size="sm"
          variant="outline"
          className="w-full gap-2 text-xs"
          onClick={handleOptimizeMemory}
          disabled={optimizing}
        >
          {optimizing ? (
            <>
              <Loader2 className="size-3.5 animate-spin" />
              {t("launch.panel.freeingSystemMemory")}
            </>
          ) : lastReport ? (
            <>
              <Sparkles className="size-3.5" />
              {t("launch.panel.freedAboutValueMbCleanAgain", { value: lastReport.freed_mb >= 0 ? lastReport.freed_mb : 0 })}
            </>
          ) : (
            <>
              <HardDrive className="size-3.5" />
              {t("launch.panel.freeMemoryClearSystemCache")}
            </>
          )}
        </Button>

        {/* 启动分析报告 */}
        <LaunchReportButton />

        {/* 内存清理报告 / 错误 */}
        <AnimatePresence>
          {lastReport && !optimizing && (
            <motion.div
              key="memory-report"
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <div className="flex items-start gap-2 rounded-xl bg-muted/50 p-3 text-[11px] text-muted-foreground">
                <Sparkles className="size-3.5 shrink-0 mt-0.5" />
                <div className="flex-1">
                  <p>
                    {t("launch.panel.availableMemory")}{lastReport.available_before_mb} MB →
                    <span className="font-semibold text-foreground">
                      {" "}
                      {lastReport.available_after_mb} MB
                    </span>{" "}
                    {t("launch.panel.totalMbMbTotal", { totalMb: lastReport.total_mb })}
                  </p>
                  <p>
                    {t("launch.panel.platform")}{lastReport.platform} · {t("launch.panel.duration")}{lastReport.duration_ms} ms
                  </p>
                  {lastReport.methods.length > 0 && (
                    <p className="mt-1 opacity-70 truncate">
                      {t("launch.panel.methods")}{lastReport.methods.join(" · ")}
                    </p>
                  )}
                </div>
              </div>
            </motion.div>
          )}

          {optimizeError && (
            <motion.div
              key="memory-error"
              variants={fadeSlideUp}
              initial="initial"
              animate="animate"
              exit="exit"
            >
              <div className="flex items-start gap-2 rounded-xl bg-destructive/10 p-3 text-[11px] text-destructive">
                <AlertCircle className="size-3.5 shrink-0 mt-0.5" />
                <span>{optimizeError}</span>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* 快捷信息 */}
        <div className="grid grid-cols-3 gap-2 text-[10px] text-muted-foreground">
          <div className="rounded-lg bg-muted/50 p-2">
            <span className="block font-medium text-foreground">{t("launch.panel.memory")}</span>
            {config.maxMemory || t("common.notSet")} MB
          </div>
          <div className="rounded-lg bg-muted/50 p-2">
            <span className="block font-medium text-foreground">Java</span>
            <span className="truncate block">
              {config.javaPath
                ? (() => {
                    const inst = javaInstallations?.[config.javaPath];
                    return inst ? `Java ${inst.major_version}` : (config.javaPath.split("/").pop() || config.javaPath.split("\\").pop());
                  })()
                : t("common.notSet")}
            </span>
          </div>
          <div className="rounded-lg bg-muted/50 p-2">
            <span className="block font-medium text-foreground">{t("launch.panel.window")}</span>
            {config.windowWidth || "873"} × {config.windowHeight || "486"}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}