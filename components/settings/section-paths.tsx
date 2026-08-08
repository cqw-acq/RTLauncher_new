"use client";

import { useEffect, useState } from "react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Coffee, FolderOpen, HardDrive, Plus, Trash2, Loader2, Check, RotateCcw, RefreshCw, Database } from "lucide-react";
import { cn } from "@/lib/utils";

interface JavaInfo {
  path: string;
  version?: string;
  vendor?: string;
}

interface LauncherPathsConfig {
  java_paths: string[];
  selected_java_path: string;
  minecraft_paths: string[];
  selected_minecraft_path: string;
  default_minecraft_path: string;
  java_installations?: Record<string, { path: string; version?: string; vendor?: string; major_version?: number; architecture?: string }>;
}

// —— 路径 & Java ——
export function PathsSection() {
  const [config, setConfig] = useState<LauncherPathsConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [newJavaPath, setNewJavaPath] = useState("");
  const [newMcPath, setNewMcPath] = useState("");

  // 加载
  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const cfg = await invoke<LauncherPathsConfig>("get_launcher_paths_config");
      setConfig(cfg);
    } catch (e) {
      // 在非 Tauri 环境（纯浏览器预览），使用示例数据
      const notInTauri =
        typeof window !== "undefined" &&
        !(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
      if (notInTauri) {
        setConfig({
          java_paths: ["C:\\Program Files\\Java\\jdk-17\\bin\\java.exe"],
          selected_java_path: "C:\\Program Files\\Java\\jdk-17\\bin\\java.exe",
          minecraft_paths: ["C:\\Users\\Me\\AppData\\Roaming\\.minecraft"],
          selected_minecraft_path: "C:\\Users\\Me\\AppData\\Roaming\\.minecraft",
          default_minecraft_path: "C:\\Users\\Me\\AppData\\Roaming\\.minecraft",
        });
      } else {
          setError(e instanceof Error ? e.message : String(e));
      }
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  // 保存
  const save = async () => {
    if (!config) return;
    setSaving(true);
    setError(null);
    setSuccess(null);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("save_launcher_paths_config", { config });
      setSuccess("配置已保存。下次启动游戏时将使用新配置。");
      setTimeout(() => setSuccess(null), 3000);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  // 选择文件/目录（Tauri dialog）
  const pickJavaExe = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const res = await open({
        title: "选择 Java 可执行文件",
        multiple: false,
        filters: [{ name: "Java 可执行文件", extensions: ["exe"] }],
      });
      if (res && typeof res === "string") setNewJavaPath(res);
    } catch (e) {
      console.warn("无法打开文件选择器", e);
    }
  };

  const pickMcDir = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const res = await open({
        title: "选择游戏目录",
        multiple: false,
        directory: true,
      });
      if (res && typeof res === "string") setNewMcPath(res);
    } catch (e) {
      console.warn("无法打开目录选择器", e);
    }
  };

  const addJava = () => {
    if (!newJavaPath.trim() || !config) return;
    if (config.java_paths.includes(newJavaPath.trim())) return;
    setConfig({
      ...config,
      java_paths: [...config.java_paths, newJavaPath.trim()],
      selected_java_path: newJavaPath.trim(),
    });
    setNewJavaPath("");
  };

  const removeJava = (p: string) => {
    if (!config) return;
    const next = config.java_paths.filter((x) => x !== p);
    setConfig({
      ...config,
      java_paths: next,
      selected_java_path: config.selected_java_path === p ? next[0] || "" : config.selected_java_path,
    });
  };

  const addMc = () => {
    if (!newMcPath.trim() || !config) return;
    if (config.minecraft_paths.includes(newMcPath.trim())) return;
    setConfig({
      ...config,
      minecraft_paths: [...config.minecraft_paths, newMcPath.trim()],
      selected_minecraft_path: newMcPath.trim(),
    });
    setNewMcPath("");
  };

  const removeMc = (p: string) => {
    if (!config) return;
    const next = config.minecraft_paths.filter((x) => x !== p);
    setConfig({
      ...config,
      minecraft_paths: next,
      selected_minecraft_path: config.selected_minecraft_path === p ? next[0] || config.default_minecraft_path : config.selected_minecraft_path,
    });
  };

  return (
    <Card id="section-paths" className="scroll-mt-4">
      <CardHeader>
        <div className="flex items-start justify-between gap-4">
          <div>
            <CardTitle className="flex items-center gap-2">
              <HardDrive className="size-4 text-primary" />
              Java 与游戏目录
            </CardTitle>
            <CardDescription>管理用于启动 Minecraft 的 Java 环境与游戏存档目录</CardDescription>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button size="sm" variant="outline" onClick={load} disabled={loading} className="gap-1.5">
              <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
              刷新
            </Button>
            <Button size="sm" onClick={save} disabled={loading || saving} className="gap-1.5">
              {saving ? <Loader2 className="size-3.5 animate-spin" /> : <Check className="size-3.5" />}
              {saving ? "保存中" : "保存"}
            </Button>
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-6">
        {error && (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}
        {success && (
          <Alert>
            <AlertDescription className="flex items-center gap-2">
              <Check className="size-4" /> {success}
            </AlertDescription>
          </Alert>
        )}

        {loading || !config ? (
          <div className="flex items-center justify-center py-8 text-sm text-muted-foreground">
            <Loader2 className="mr-2 size-4 animate-spin" />
            正在读取配置...
          </div>
        ) : (
          <>
            {/* Java */}
            <div className="space-y-3">
              <Label className="flex items-center gap-2">
                <Coffee className="size-3.5 text-muted-foreground" />
                Java 可执行文件
              </Label>
              <div className="flex flex-col gap-2">
                <Select
                  value={config.selected_java_path}
                  onValueChange={(v) => setConfig({ ...config, selected_java_path: v })}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="请选择 Java 路径" />
                  </SelectTrigger>
                  <SelectContent>
                    {config.java_paths.length === 0 ? (
                      <SelectItem value="__empty__" disabled>
                        暂无，请添加
                      </SelectItem>
                    ) : (
                      config.java_paths.map((p) => (
                        <SelectItem key={p} value={p} className="truncate font-mono text-xs">
                          {p}
                        </SelectItem>
                      ))
                    )}
                  </SelectContent>
                </Select>

                <div className="flex items-center gap-2">
                  <Input
                    value={newJavaPath}
                    onChange={(e) => setNewJavaPath(e.target.value)}
                    placeholder="例如：C:\\Program Files\\Java\\jdk-17\\bin\\java.exe"
                    className="font-mono text-xs"
                  />
                  <Button type="button" variant="outline" size="sm" onClick={pickJavaExe} className="gap-1.5">
                    <FolderOpen className="size-3.5" />
                    浏览
                  </Button>
                  <Button type="button" size="sm" onClick={addJava} disabled={!newJavaPath.trim()} className="gap-1.5">
                    <Plus className="size-3.5" />
                    添加
                  </Button>
                </div>

                {config.java_paths.length > 0 && (
                  <div className="flex flex-wrap gap-1.5">
                    {config.java_paths.map((p) => (
                      <span
                        key={p}
                        className={cn(
                          "inline-flex max-w-full items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs transition-colors",
                          config.selected_java_path === p
                            ? "border-primary bg-primary/10 text-primary"
                            : "border-border bg-card hover:bg-accent/30"
                        )}
                      >
                        <span className="max-w-[36ch] truncate font-mono">{p}</span>
                        <button
                          type="button"
                          onClick={() => removeJava(p)}
                          className="text-muted-foreground transition-colors hover:text-destructive"
                          title="移除"
                          aria-label="移除"
                        >
                          <Trash2 className="size-3" />
                        </button>
                      </span>
                    ))}
                  </div>
                )}
                <p className="text-xs text-muted-foreground">推荐 Minecraft 1.17+ 使用 Java 17 或更高版本。</p>
              </div>
            </div>

            {/* 游戏目录 */}
            <div className="space-y-3">
              <Label className="flex items-center gap-2">
                <Database className="size-3.5 text-muted-foreground" />
                游戏目录
              </Label>
              <div className="flex flex-col gap-2">
                <Select
                  value={config.selected_minecraft_path}
                  onValueChange={(v) => setConfig({ ...config, selected_minecraft_path: v })}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder="请选择游戏目录" />
                  </SelectTrigger>
                  <SelectContent>
                    {config.minecraft_paths.map((p) => (
                      <SelectItem key={p} value={p} className="truncate font-mono text-xs">
                        {p}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                <div className="flex items-center gap-2">
                  <Input
                    value={newMcPath}
                    onChange={(e) => setNewMcPath(e.target.value)}
                    placeholder="例如：C:\\Users\\Me\\AppData\\Roaming\\.minecraft"
                    className="font-mono text-xs"
                  />
                  <Button type="button" variant="outline" size="sm" onClick={pickMcDir} className="gap-1.5">
                    <FolderOpen className="size-3.5" />
                    浏览
                  </Button>
                  <Button type="button" size="sm" onClick={addMc} disabled={!newMcPath.trim()} className="gap-1.5">
                    <Plus className="size-3.5" />
                    添加
                  </Button>
                </div>

                {config.minecraft_paths.length > 0 && (
                  <div className="flex flex-wrap gap-1.5">
                    {config.minecraft_paths.map((p) => (
                      <span
                        key={p}
                        className={cn(
                          "inline-flex max-w-full items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs transition-colors",
                          config.selected_minecraft_path === p
                            ? "border-primary bg-primary/10 text-primary"
                            : "border-border bg-card hover:bg-accent/30"
                        )}
                      >
                        <span className="max-w-[36ch] truncate font-mono">{p}</span>
                        <button
                          type="button"
                          onClick={() => removeMc(p)}
                          className="text-muted-foreground transition-colors hover:text-destructive"
                          title="移除"
                          aria-label="移除"
                        >
                          <Trash2 className="size-3" />
                        </button>
                      </span>
                    ))}
                  </div>
                )}
                <p className="text-xs text-muted-foreground">
                  默认目录：<span className="font-mono">{config.default_minecraft_path}</span>
                </p>
              </div>
            </div>

            <div className="flex justify-end">
              <Button variant="outline" size="sm" onClick={load} className="gap-1.5">
                <RotateCcw className="size-3.5" />
                放弃修改
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}