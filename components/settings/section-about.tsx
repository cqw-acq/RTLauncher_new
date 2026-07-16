"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Loader2, Download, Check, RefreshCcw, Package, Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "available"; version: string; notes: string }
  | { kind: "up-to-date" }
  | { kind: "error"; message: string };

export function AboutSection() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });
  const [installing, setInstalling] = useState(false);
  const [version, setVersion] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const { getVersion } = await import("@tauri-apps/api/app");
        const v = await getVersion();
        setVersion(v);
      } catch {
        setVersion("dev");
      }
    })();
  }, []);

  const check = async () => {
    setState({ kind: "checking" });
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (update?.available) {
        setState({
          kind: "available",
          version: update.version ?? "",
          notes: (update.body ?? "").toString(),
        });
      } else {
        setState({ kind: "up-to-date" });
      }
    } catch (e) {
      setState({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    }
  };

  const install = async () => {
    setInstalling(true);
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (!update?.available) return;
      await update.downloadAndInstall(() => {});
    } catch (e) {
      setState({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    } finally {
      setInstalling(false);
    }
  };

  // 首次进入自动检查一次
  useEffect(() => {
    const t = setTimeout(() => {
      if (state.kind === "idle") check();
    }, 800);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <Card id="section-about" className="scroll-mt-4">
      <CardHeader className="pb-4">
        <div>
          <CardTitle className="flex items-center gap-2 text-base">
            <Package className="size-4 text-primary" />
            版本更新
          </CardTitle>
          <CardDescription className="text-xs mt-1">当前版本、检查启动器更新</CardDescription>
        </div>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center gap-3 rounded-lg border border-border bg-card p-3">
          <div className="flex size-8 items-center justify-center rounded-lg bg-primary/10 text-primary shrink-0">
            <Sparkles className="size-4" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-sm font-medium">RTLauncher</div>
            <div className="text-xs text-muted-foreground">
              版本 <span className="font-mono">{version ?? "—"}</span>
            </div>
          </div>
        </div>

        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-sm">
              <RefreshCcw className={cn("size-3.5 text-muted-foreground", state.kind === "checking" && "animate-spin")} />
              {state.kind === "idle" && "尚未检查更新"}
              {state.kind === "checking" && "正在检查更新..."}
              {state.kind === "up-to-date" && <span className="text-emerald-600 dark:text-emerald-400">已是最新版本</span>}
              {state.kind === "available" && (
                <span className="text-amber-600 dark:text-amber-400">发现新版本 v{state.version}</span>
              )}
              {state.kind === "error" && <span className="text-destructive">检查失败</span>}
            </div>
            <div className="flex items-center gap-2">
              <Button size="sm" variant="outline" onClick={check} disabled={state.kind === "checking"} className="gap-1.5 h-8">
                <RefreshCcw className={cn("size-3.5", state.kind === "checking" && "animate-spin")} />
                检查更新
              </Button>
              {state.kind === "available" && (
                <Button size="sm" onClick={install} disabled={installing} className="gap-1.5 h-8">
                  {installing ? <Loader2 className="size-3.5 animate-spin" /> : <Download className="size-3.5" />}
                  {installing ? "安装中" : "立即安装"}
                </Button>
              )}
            </div>
          </div>

          {state.kind === "available" && state.notes && (
            <pre className="max-h-56 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-border bg-muted/30 p-3 text-xs leading-relaxed text-foreground/80">
              {state.notes}
            </pre>
          )}

          {state.kind === "error" && (
            <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-3 text-xs text-destructive">
              {state.message}
            </div>
          )}

          {state.kind === "up-to-date" && (
            <div className="flex items-center gap-2 rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-3 text-xs text-emerald-600 dark:text-emerald-400">
              <Check className="size-3.5" />
              启动器已在最新版本。
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}