"use client";

import { useState, useEffect } from "react";
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

export default function CheckUpdatePage() {
  const router = useRouter();
  const { t } = useI18n();
  const [currentVersion, setCurrentVersion] = useState<string>("");
  const [info, setInfo] = useState<UpdateInfo>({
    status: "idle",
    canCheck: true,
  });

  useEffect(() => {
    loadVersion();
    handleCheck();
  }, []);

  const loadVersion = async () => {
    try {
      const v = await getVersion();
      setCurrentVersion(v);
    } catch {
      setCurrentVersion("dev");
    }
  };

  const handleCheck = async () => {
    setInfo((prev) => ({ ...prev, status: "checking", errorMessage: undefined }));
    try {
      const result = await invoke<UpdateCheckResult>("check_for_updates");
      
      if (!result.needs_check) {
        setInfo((prev) => ({
          ...prev,
          status: prev.status === "idle" ? "up-to-date" : prev.status,
          canCheck: false,
          errorMessage: result.message,
          lastCheckTime: Math.floor(Date.now() / 1000),
        }));
        return;
      }

      if (result.update_available && result.target_version) {
        setInfo({
          status: "available",
          version: result.target_version,
          canCheck: true,
          lastCheckTime: Math.floor(Date.now() / 1000),
        });
      } else {
        setInfo({
          status: "up-to-date",
          canCheck: true,
          lastCheckTime: Math.floor(Date.now() / 1000),
        });
      }
    } catch (e) {
      setInfo({
        status: "error",
        errorMessage: e instanceof Error ? e.message : String(e),
        canCheck: true,
      });
    }
  };

  const handleDownload = async () => {
    setInfo((prev) => ({ ...prev, status: "downloading", errorMessage: undefined }));
    try {
      const result = await invoke<{ success: boolean; path: string; size: number }>("download_update");
      if (result.success) {
        setInfo((prev) => ({ ...prev, status: "downloaded" }));
      } else {
        setInfo({
          status: "error",
          errorMessage: "下载失败",
          canCheck: true,
        });
      }
    } catch (e) {
      setInfo({
        status: "error",
        errorMessage: e instanceof Error ? e.message : String(e),
        canCheck: true,
      });
    }
  };

  const handleInstall = async () => {
    setInfo((prev) => ({ ...prev, status: "installing", errorMessage: undefined }));
    try {
      await invoke("install_update");
    } catch (e) {
      setInfo({
        status: "error",
        errorMessage: e instanceof Error ? e.message : String(e),
        canCheck: true,
      });
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

              {(info.status === "downloading" || info.status === "installing") && (
                <div className="rounded-lg border border-primary/30 bg-primary/5 p-4">
                  <div className="flex items-center gap-2">
                    <Loader2 className="size-4 animate-spin text-primary" />
                    <span className="text-sm">
                      {info.status === "downloading"
                        ? t("checkUpdate.downloadingUpdate")
                        : t("checkUpdate.installingUpdate")}
                    </span>
                  </div>
                </div>
              )}

              <div className="flex flex-wrap items-center gap-2 pt-2">
                <Button
                  variant="outline"
                  onClick={handleCheck}
                  disabled={!canCheckNow}
                  className="gap-1.5"
                >
                  <RefreshCcw className={`size-3.5 ${info.status === "checking" ? "animate-spin" : ""}`} />
                  {t("checkUpdate.checkForUpdates")}
                </Button>

                {(info.status === "available" || info.status === "downloading") && (
                  <Button
                    onClick={handleDownload}
                    disabled={info.status === "downloading"}
                    className="gap-1.5"
                  >
                    <Download className="size-3.5" />
                    {t("checkUpdate.downloadUpdate")}
                  </Button>
                )}

                {(info.status === "downloaded" || info.status === "installing") && (
                  <Button
                    onClick={handleInstall}
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