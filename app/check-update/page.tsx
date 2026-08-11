"use client";

import { useState, useEffect, useRef } from "react";
import { useRouter } from "next/navigation";
import {
  ArrowLeft,
  Check,
  Download,
  Loader2,
  RefreshCcw,
  Package,
  Sparkles,
  AlertCircle,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useI18n } from "@/components/i18n/use-i18n";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";

type UpdateStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "downloaded"
  | "installing"
  | "up-to-date"
  | "error";

interface UpdateInfo {
  status: UpdateStatus;
  version?: string;
  errorMessage?: string;
  canCheck: boolean;
  lastCheckTime?: number;
}

interface UpdateCheckResult {
  needs_check: boolean;
  update_available: boolean;
  current_version: string;
  target_version: string | null;
  message: string;
}

function readAutoStartFlag(): boolean {
  if (typeof window === "undefined") return false;
  const sp = new URLSearchParams(window.location.search);
  return sp.get("autoStart") === "1";
}

export default function CheckUpdatePage() {
  const router = useRouter();
  const [autoStart] = useState<boolean>(() => readAutoStartFlag());
  const autoStartedRef = useRef(false);

  const { t } = useI18n();
  const [currentVersion, setCurrentVersion] = useState<string>("");
  const [info, setInfo] = useState<UpdateInfo>({
    status: "idle",
    canCheck: true,
  });
  const [downloadProgress, setDownloadProgress] = useState<number | null>(null);

  useEffect(() => {
    loadVersion();
    handleCheck(autoStart).then(async (hadUpdate) => {
      if (autoStart && hadUpdate) {
        // 启动自动下载 → 下载完成自动安装
        autoStartedRef.current = true;
        const downloadOk = await handleDownload();
        if (downloadOk) {
          // 小延迟让用户看到"已下载"状态
          setTimeout(() => handleInstall(), 600);
        }
      }
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (info.status !== "downloading") return;
    let timer: ReturnType<typeof setInterval>;
    const poll = async () => {
      try {
        const cfg = await invoke<{
          status: string;
          download_progress?: number;
          download_path?: string;
        }>("get_update_status");
        if (typeof cfg.download_progress === "number") {
          setDownloadProgress(cfg.download_progress);
        }
        if (cfg.status === "downloaded") {
          setDownloadProgress(100);
          clearInterval(timer);
        }
      } catch {
        // 忽略轮询错误
      }
    };
    poll();
    timer = setInterval(poll, 500);
    return () => clearInterval(timer);
  }, [info.status]);

  const loadVersion = async () => {
    try {
      const v = await getVersion();
      setCurrentVersion(v);
    } catch {
      setCurrentVersion("dev");
    }
  };

  /**
   * @param force 是否强制跳过 24 小时间隔（启动自动跳转时传 true）
   * @returns 是否发现有新版本可用
   */
  const handleCheck = async (force = false): Promise<boolean> => {
    setInfo((prev) => ({ ...prev, status: "checking", errorMessage: undefined }));
    try {
      const result = await invoke<UpdateCheckResult>("check_for_updates", { force });

      if (!result.needs_check && !force) {
        setInfo((prev) => ({
          ...prev,
          status: prev.status === "idle" ? "up-to-date" : prev.status,
          canCheck: false,
          errorMessage: result.message,
          lastCheckTime: Math.floor(Date.now() / 1000),
        }));
        return false;
      }

      if (result.update_available && result.target_version) {
        setInfo({
          status: "available",
          version: result.target_version,
          canCheck: true,
          lastCheckTime: Math.floor(Date.now() / 1000),
        });
        return true;
      } else {
        setInfo({
          status: "up-to-date",
          canCheck: true,
          lastCheckTime: Math.floor(Date.now() / 1000),
        });
        return false;
      }
    } catch (e) {
      setInfo({
        status: "error",
        errorMessage: e instanceof Error ? e.message : String(e),
        canCheck: true,
      });
      return false;
    }
  };

  const handleDownload = async (): Promise<boolean> => {
    setDownloadProgress(0);
    setInfo((prev) => ({ ...prev, status: "downloading", errorMessage: undefined }));
    try {
      const result = await invoke<{ success: boolean; path: string; size: number }>("download_update");
      if (result.success) {
        setDownloadProgress(100);
        setInfo((prev) => ({ ...prev, status: "downloaded" }));
        return true;
      } else {
        setInfo({
          status: "error",
          errorMessage: "下载失败",
          canCheck: true,
        });
        return false;
      }
    } catch (e) {
      setInfo({
        status: "error",
        errorMessage: e instanceof Error ? e.message : String(e),
        canCheck: true,
      });
      return false;
    }
  };

  const handleInstall = async (): Promise<boolean> => {
    setInfo((prev) => ({ ...prev, status: "installing", errorMessage: undefined }));
    try {
      await invoke("install_update");
      return true;
    } catch (e) {
      setInfo({
        status: "error",
        errorMessage: e instanceof Error ? e.message : String(e),
        canCheck: true,
      });
      return false;
    }
  };

  const statusIcon = () => {
    switch (info.status) {
      case "checking":
        return <Loader2 className="size-5 animate-spin text-primary" />;
      case "available":
        return <Sparkles className="size-5 text-amber-500" />;
      case "downloading":
        return <Loader2 className="size-5 animate-spin text-sky-500" />;
      case "downloaded":
        return <Download className="size-5 text-emerald-500" />;
      case "installing":
        return <Loader2 className="size-5 animate-spin text-emerald-500" />;
      case "up-to-date":
        return <Check className="size-5 text-emerald-500" />;
      case "error":
        return <AlertCircle className="size-5 text-destructive" />;
      default:
        return <Package className="size-5 text-muted-foreground" />;
    }
  };

  const statusText = () => {
    switch (info.status) {
      case "idle":
        return t("checkUpdate.idle");
      case "checking":
        return t("checkUpdate.checking");
      case "available":
        return t("checkUpdate.available");
      case "downloading":
        return t("checkUpdate.downloading");
      case "downloaded":
        return t("checkUpdate.downloaded");
      case "installing":
        return t("checkUpdate.installing");
      case "up-to-date":
        return t("checkUpdate.upToDate");
      case "error":
        return t("checkUpdate.error");
    }
  };

  const canCheckNow = info.canCheck && info.status !== "checking" && info.status !== "downloading" && info.status !== "installing";

  return (
    <div className="flex flex-col h-full bg-background">
      <div className="shrink-0 border-b border-border p-4">
        <div className="flex items-center gap-3">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => router.back()}
            className="shrink-0"
          >
            <ArrowLeft className="size-4" />
          </Button>
          <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
            <Package className="size-5 text-primary" />
          </div>
          <div>
            <h1 className="text-lg font-semibold leading-none">{t("checkUpdate.title")}</h1>
            <p className="mt-1 text-xs text-muted-foreground">{t("checkUpdate.subtitle")}</p>
          </div>
        </div>
      </div>

      <div className="flex-1 p-4 md:p-6 overflow-y-auto">
        <div className="max-w-2xl mx-auto space-y-4">
          <Card>
            <CardHeader className="pb-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="flex size-12 items-center justify-center rounded-xl bg-primary/10">
                    {statusIcon()}
                  </div>
                  <div>
                    <CardTitle className="text-base">RTLauncher</CardTitle>
                    <p className="text-xs text-muted-foreground mt-0.5">
                      {t("checkUpdate.currentVersion")} <span className="font-mono">{currentVersion}</span>
                    </p>
                  </div>
                </div>
                <Badge
                  variant={
                    info.status === "available" || info.status === "downloaded"
                      ? "default"
                      : info.status === "up-to-date"
                      ? "secondary"
                      : "outline"
                  }
                  className="text-xs"
                >
                  {statusText()}
                </Badge>
              </div>
            </CardHeader>

            <CardContent className="space-y-4">
              {info.status === "available" && info.version && (
                <div className="rounded-lg border border-amber-500/30 bg-amber-500/5 p-4">
                  <div className="flex items-center gap-2 mb-1">
                    <Sparkles className="size-4 text-amber-500" />
                    <span className="text-sm font-medium">
                      {t("checkUpdate.newVersionAvailable")} v{info.version}
                    </span>
                  </div>
                </div>
              )}

              {info.status === "downloaded" && info.version && (
                <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-4">
                  <div className="flex items-center gap-2">
                    <Download className="size-4 text-emerald-500" />
                    <span className="text-sm font-medium">
                      {t("checkUpdate.downloadedReady")}
                    </span>
                  </div>
                </div>
              )}

              {info.status === "up-to-date" && (
                <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-4">
                  <div className="flex items-center gap-2">
                    <Check className="size-4 text-emerald-500" />
                    <span className="text-sm">{t("checkUpdate.upToDateMessage")}</span>
                  </div>
                </div>
              )}

              {info.status === "error" && info.errorMessage && (
                <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4">
                  <div className="flex items-start gap-2">
                    <AlertCircle className="size-4 text-destructive mt-0.5" />
                    <div>
                      <span className="text-sm font-medium text-destructive">
                        {t("checkUpdate.checkFailed")}
                      </span>
                      <p className="text-xs text-destructive/80 mt-1">{info.errorMessage}</p>
                    </div>
                  </div>
                </div>
              )}

              {info.status === "downloading" && (
                <div className="rounded-lg border border-sky-500/30 bg-sky-500/5 p-4 space-y-3">
                  <div className="flex items-center gap-2">
                    <Loader2 className="size-4 animate-spin text-sky-500" />
                    <span className="text-sm font-medium">
                      {t("checkUpdate.downloadingUpdate")}
                      {typeof downloadProgress === "number" && (
                        <span className="ml-2 font-mono text-sky-600">
                          {downloadProgress.toFixed(1)}%
                        </span>
                      )}
                    </span>
                  </div>
                  {typeof downloadProgress === "number" && (
                    <div className="w-full h-2 rounded-full bg-sky-500/15 overflow-hidden">
                      <div
                        className="h-full bg-sky-500 transition-all duration-300"
                        style={{ width: `${Math.min(100, downloadProgress)}%` }}
                      />
                    </div>
                  )}
                </div>
              )}

              {info.status === "installing" && (
                <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-4">
                  <div className="flex items-center gap-2">
                    <Loader2 className="size-4 animate-spin text-emerald-500" />
                    <span className="text-sm font-medium">
                      {t("checkUpdate.installingUpdate")}
                    </span>
                  </div>
                  <p className="text-xs text-muted-foreground mt-2">
                    {t("checkUpdate.willRestartShortly")}
                  </p>
                </div>
              )}

              <div className="flex flex-wrap items-center gap-2 pt-2">
                <Button
                  variant="outline"
                  onClick={() => handleCheck()}
                  disabled={!canCheckNow}
                  className="gap-1.5"
                >
                  <RefreshCcw className={`size-3.5 ${info.status === "checking" ? "animate-spin" : ""}`} />
                  {t("checkUpdate.checkForUpdates")}
                </Button>

                {(info.status === "available" || info.status === "downloading") && (
                  <Button
                    onClick={() => handleDownload()}
                    disabled={info.status === "downloading"}
                    className="gap-1.5"
                  >
                    <Download className="size-3.5" />
                    {t("checkUpdate.downloadUpdate")}
                  </Button>
                )}

                {(info.status === "downloaded" || info.status === "installing") && (
                  <Button
                    onClick={() => handleInstall()}
                    disabled={info.status === "installing"}
                    className="gap-1.5"
                  >
                    <Check className="size-3.5" />
                    {t("checkUpdate.installNow")}
                  </Button>
                )}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardContent className="pt-6">
              <div className="text-xs text-muted-foreground space-y-2">
                <p>{t("checkUpdate.updateIntervalNotice")}</p>
                {info.lastCheckTime && (
                  <p>
                    {t("checkUpdate.lastCheckTime")}:{" "}
                    {new Date(info.lastCheckTime * 1000).toLocaleString()}
                  </p>
                )}
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}