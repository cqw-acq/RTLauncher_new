"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { Download, Check, Loader2, RefreshCw, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { check } from "@tauri-apps/plugin-updater";
import { useI18n } from "@/components/i18n/use-i18n";
import type { AppLanguage } from "@/components/settings/settings-provider";

/**
 * 自动更新组件 — 提供共享更新状态、主页提示和手动更新操作。
 * 启动检查及 changelog 弹窗由 StartupUpdateNotifier 负责。
 *
 * 通过 AppUpdateProvider 共享状态，AppUpdateBadge 在主页显示小的更新提示。
 */

export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; notes: string; prepared: boolean }
  | { kind: "up-to-date" }
  | { kind: "error"; message: string };

// ---- Provider：全局共享更新状态 ----

let _state: UpdateState = { kind: "idle" };
const _listeners = new Set<(s: UpdateState) => void>();

export function setAppUpdateState(s: UpdateState) {
  _state = s;
  for (const l of _listeners) l(s);
}

function updateAlert(language: AppLanguage, chinese: string, english: string) {
  return language === "en-US" ? english : chinese;
}

async function checkInBackground(showError = false, language: AppLanguage = "zh-CN") {
  setAppUpdateState({ kind: "checking" });
  try {
    const update = await check();
    if (update?.available) {
      setAppUpdateState({
        kind: "available",
        version: update.version ?? "",
        notes: (update.body ?? "").toString(),
        prepared: false,
      });
    } else {
      setAppUpdateState({ kind: "up-to-date" });
      if (showError) {
        setTimeout(() => {
          window.alert(updateAlert(language, "当前已是最新版本 ✅", "You're up to date ✅"));
        }, 50);
      }
    }
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    setAppUpdateState({ kind: "error", message: msg });
    if (showError) {
      setTimeout(() => {
        window.alert(updateAlert(language, "检查更新失败：\n", "Update check failed:\n") + msg);
      }, 50);
    }
  }
}

// 手动触发下载安装（弹窗模式用）
async function downloadAndInstall(language: AppLanguage = "zh-CN"): Promise<boolean> {
  try {
    const update = await check();
    if (!update?.available) {
      window.alert(updateAlert(language, "没有可用更新", "No updates are available"));
      return false;
    }
    await update.downloadAndInstall(() => {});
    window.alert(updateAlert(language, "新版本下载完成，即将重启并安装。", "The update has downloaded and will restart to install."));
    return true;
  } catch (e) {
    console.error("[updater] install failed:", e);
    window.alert(
      updateAlert(language, "安装失败：\n", "Installation failed:\n") + (e instanceof Error ? e.message : String(e))
    );
    return false;
  }
}

// ---- Hook：读取共享状态并注册重渲染 ----

function useUpdateState(): UpdateState {
  const [, forceRender] = useState(0);
  useEffect(() => {
    const l: (s: UpdateState) => void = () => forceRender((n) => n + 1);
    _listeners.add(l);
    l(_state);
    return () => {
      _listeners.delete(l);
    };
  }, []);
  return _state;
}

// ---- 组件 A：主页显示的小提示条（ProfileCard 底部使用）----

export function AppUpdateBadge() {
  const state = useUpdateState();
  const { t } = useI18n();

  if (state.kind === "available") {
    return (
      <AppUpdateDialogTrigger
        version={state.version}
        notes={state.notes}
        prepared={state.prepared}
      />
    );
  }

  if (state.kind === "checking") {
    return (
      <div className="flex items-center justify-center gap-2 px-3 py-2 text-xs text-muted-foreground">
        <Loader2 className="size-3.5 animate-spin" />
        {t("settings.appUpdater.checkingForUpdates")}
      </div>
    );
  }

  // 空闲 / 已是最新 / 错误 — 不显示任何内容（避免打扰）
  return null;
}

// ---- 组件 B：一个按钮，点击后进行完整的"检查 → 弹窗 → 下载"流程 ----

export function AppUpdateButton({
  variant = "button",
}: {
  variant?: "button" | "banner";
}) {
  const state = useUpdateState();
  const { t, language } = useI18n();
  const [dialogOpen, setDialogOpen] = useState(false);

  async function handleCheck() {
    await checkInBackground(true, language);
    if (_state.kind === "available") {
      setDialogOpen(true);
    }
  }

  const [installing, setInstalling] = useState(false);
  async function handleInstall() {
    setInstalling(true);
    const ok = await downloadAndInstall(language);
    if (!ok) setInstalling(false);
  }

  return (
    <>
      {variant === "button" ? (
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="shrink-0 gap-1.5"
          onClick={handleCheck}
          disabled={state.kind === "checking"}
          title={t("settings.appUpdater.checkForLauncherUpdates")}
        >
          {state.kind === "checking" ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : state.kind === "available" ? (
            <Download className="size-3.5" />
          ) : (
            <RefreshCw className="size-3.5" />
          )}
          <span className="text-xs">{t("settings.appUpdater.checkForUpdates")}</span>
        </Button>
      ) : (
        <button
          type="button"
          onClick={() => setDialogOpen(true)}
          className="w-full flex items-center justify-center gap-2 px-3 py-2 text-xs rounded-md bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 transition-colors"
        >
          <Sparkles className="size-3.5" />
          {t("settings.appUpdater.newVersionVValueViewDetails", { value: state.kind === "available" ? state.version : "" })}
        </button>
      )}

      {state.kind === "available" && (
        <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
          <DialogContent className="sm:max-w-lg">
            <DialogHeader>
              <DialogTitle className="flex items-center gap-2">
                <Download className="size-5" />
                {t("settings.appUpdater.newVersionVVersion", { version: state.version })}
              </DialogTitle>
            </DialogHeader>

            <div className="px-5 py-4">
              {state.notes ? (
                <pre className="max-h-52 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 text-xs leading-relaxed text-foreground/80">
                  {state.notes}
                </pre>
              ) : (
                <span className="text-sm text-muted-foreground">{t("settings.appUpdater.noReleaseNotesAvailable")}</span>
              )}

              {installing && (
                <div className="mt-4 flex items-center gap-3">
                  <Loader2 className="size-4 animate-spin" />
                  <span className="text-xs text-muted-foreground">{t("settings.appUpdater.downloadingAndInstalling")}</span>
                </div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border">
              <Button
                variant="outline"
                onClick={() => setDialogOpen(false)}
                disabled={installing}
              >
                {t("settings.appUpdater.later")}
              </Button>
              <Button onClick={handleInstall} disabled={installing}>
                {installing ? (
                  <>
                    <Loader2 className="size-4 animate-spin" />
                    {t("settings.appUpdater.updating")}
                  </>
                ) : (
                  <>
                    <Check className="size-4" />
                    {t("settings.appUpdater.installNow")}
                  </>
                )}
              </Button>
            </div>
          </DialogContent>
        </Dialog>
      )}
    </>
  );
}

// ---- 内部组件：有新版本时的 banner 点击触发器 ----

function AppUpdateDialogTrigger({
  version,
  notes,
  prepared,
}: {
  version: string;
  notes: string;
  prepared: boolean;
}) {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [installing, setInstalling] = useState(false);
  const router = useRouter();
  const { t, language } = useI18n();

  async function handleInstall() {
    if (prepared) {
      setDialogOpen(false);
      router.push(
        `/check-update?autoStart=1&preparedVersion=${encodeURIComponent(version)}`,
      );
      return;
    }

    setInstalling(true);
    const ok = await downloadAndInstall(language);
    if (!ok) setInstalling(false);
  }

  return (
    <>
      <button
        type="button"
        onClick={() => setDialogOpen(true)}
        className="w-full flex items-center justify-center gap-2 px-3 py-2 text-xs rounded-md bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 transition-colors"
      >
        <Sparkles className="size-3.5" />
        {t("settings.appUpdater.newVersionVVersionViewDetails", { version: version })}
      </button>

      <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Download className="size-5" />
              {t("settings.appUpdater.newVersionVVersion", { version: version })}
            </DialogTitle>
          </DialogHeader>

          <div className="px-5 py-4">
            {notes ? (
              <pre className="max-h-52 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 text-xs leading-relaxed text-foreground/80">
                {notes}
              </pre>
            ) : (
              <span className="text-sm text-muted-foreground">{t("settings.appUpdater.noReleaseNotesAvailable")}</span>
            )}

            {installing && (
              <div className="mt-4 flex items-center gap-3">
                <Loader2 className="size-4 animate-spin" />
                <span className="text-xs text-muted-foreground">{t("settings.appUpdater.downloadingAndInstalling")}</span>
              </div>
            )}

          </div>

          <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border">
            <Button
              variant="outline"
              onClick={() => setDialogOpen(false)}
              disabled={installing}
            >
              {t("settings.appUpdater.later")}
            </Button>
            <Button onClick={handleInstall} disabled={installing}>
              {installing ? (
                <>
                  <Loader2 className="size-4 animate-spin" />
                  {t("settings.appUpdater.updating")}
                </>
              ) : (
                <>
                  <Check className="size-4" />
                  {t("settings.appUpdater.installNow")}
                </>
              )}
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}

// ---- 用于主页的更新提示条 ----

export function AppUpdateSection() {
  return <AppUpdateBadge />;
}
