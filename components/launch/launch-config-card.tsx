"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useLaunchContext } from "@/components/launch/launch-provider";
import {
  FolderOpen,
  HardDrive,
  Cpu,
  Package,
  User,
  Monitor,
  Plus,
  X,
  CheckCircle2,
  Circle,
} from "lucide-react";
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LauncherPathsConfig } from "@/types";

interface MemoryInfo {
  totalMB: number;
  usedMB: number;
}

function useSystemMemory(): MemoryInfo {
  const [info, setInfo] = useState<MemoryInfo>({ totalMB: 0, usedMB: 0 });

  useEffect(() => {
    invoke<{ total_mb: number; used_mb: number }>("get_system_memory")
      .then(({ total_mb, used_mb }) =>
        setInfo({ totalMB: total_mb, usedMB: used_mb })
      )
      .catch(() => {});
  }, []);

  return info;
}

/** 路径列表条目 */
function PathItem({
  path,
  isSelected,
  isDefault,
  canRemove,
  onSelect,
  onRemove,
}: {
  path: string;
  isSelected: boolean;
  isDefault?: boolean;
  canRemove: boolean;
  onSelect: () => void;
  onRemove: () => void;
}) {
  return (
    <div
      className={`flex items-center gap-1.5 rounded-md px-2 py-1 cursor-pointer transition-colors text-xs group ${
        isSelected
          ? "bg-primary/10 text-primary"
          : "hover:bg-muted text-muted-foreground"
      }`}
      onClick={onSelect}
    >
      {isSelected ? (
        <CheckCircle2 className="size-3 shrink-0 text-primary" />
      ) : (
        <Circle className="size-3 shrink-0 opacity-40" />
      )}
      <span className="flex-1 break-all leading-snug">{path}</span>
      {isDefault && (
        <span className="shrink-0 rounded px-1 py-0.5 text-[9px] font-medium bg-muted text-muted-foreground leading-none">
          默认
        </span>
      )}
      {canRemove && (
        <button
          className="shrink-0 opacity-0 group-hover:opacity-100 transition-opacity hover:text-destructive"
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
        >
          <X className="size-3" />
        </button>
      )}
    </div>
  );
}

/**
 * 启动配置卡片
 * 设置 Java 路径、内存、版本等启动参数
 */
export function LaunchConfigCard() {
  const { config, updateConfig } = useLaunchContext();
  const { totalMB, usedMB } = useSystemMemory();

  const [pathsCfg, setPathsCfg] = useState<LauncherPathsConfig>({
    java_paths: [],
    selected_java_path: "",
    minecraft_paths: [],
    selected_minecraft_path: "",
    default_minecraft_path: "",
  });

  useEffect(() => {
    invoke<LauncherPathsConfig>("get_launcher_paths_config")
      .then((cfg) => setPathsCfg(cfg))
      .catch(() => {});
  }, []);

  const savePaths = useCallback(
    async (next: LauncherPathsConfig) => {
      setPathsCfg(next);
      try {
        await invoke("save_launcher_paths_config", { config: next });
      } catch { /* ignore */ }
      if (next.selected_java_path) updateConfig({ javaPath: next.selected_java_path });
      if (next.selected_minecraft_path) updateConfig({ minecraftPath: next.selected_minecraft_path });
    },
    [updateConfig]
  );

  const openDialog = async (mode: "java" | "minecraft") => {
    try {
      const mod = await import("@tauri-apps/plugin-dialog" as string);
      const open = mod.open;
      const result =
        mode === "minecraft"
          ? await open({ directory: true, multiple: false })
          : await open({ multiple: false, filters: [{ name: "Executable", extensions: ["exe", ""] }] });
      if (!result) return;
      const path = result as string;
      if (mode === "java") {
        if (pathsCfg.java_paths.includes(path)) return;
        await savePaths({
          ...pathsCfg,
          java_paths: [...pathsCfg.java_paths, path],
          selected_java_path: path,
        });
      } else {
        if (pathsCfg.minecraft_paths.includes(path)) return;
        await savePaths({
          ...pathsCfg,
          minecraft_paths: [...pathsCfg.minecraft_paths, path],
          selected_minecraft_path: path,
        });
      }
    } catch { /* dialog 不可用或用户取消 */ }
  };

  const handleOpenFileDialog = async (field: "wrapperPath") => {
    try {
      const mod = await import("@tauri-apps/plugin-dialog" as string);
      const open = mod.open;
      const result = await open({
        multiple: false,
        filters: [{ name: "Executable", extensions: ["jar", "exe", ""] }],
      });
      if (result) updateConfig({ [field]: result as string });
    } catch { /* ignore */ }
  };

  return (
    <Card size="sm">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-sm">
          <Package className="size-4 text-primary" />
          启动配置
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* 游戏目录 — 路径列表 */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <Label className="text-xs text-muted-foreground">
              <HardDrive className="size-3" />
              游戏目录
            </Label>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-[10px] text-muted-foreground gap-1"
              onClick={() => openDialog("minecraft")}
            >
              <Plus className="size-3" /> 添加
            </Button>
          </div>
          {/* 默认路径始终显示 */}
          {pathsCfg.default_minecraft_path && (
            <PathItem
              path={pathsCfg.default_minecraft_path}
              isSelected={pathsCfg.selected_minecraft_path === pathsCfg.default_minecraft_path}
              isDefault
              canRemove={false}
              onSelect={() =>
                savePaths({ ...pathsCfg, selected_minecraft_path: pathsCfg.default_minecraft_path })
              }
              onRemove={() => {}}
            />
          )}
          {/* 用户手动添加的路径 */}
          {pathsCfg.minecraft_paths
            .filter((p) => p !== pathsCfg.default_minecraft_path)
            .map((p) => (
              <PathItem
                key={p}
                path={p}
                isSelected={pathsCfg.selected_minecraft_path === p}
                canRemove
                onSelect={() => savePaths({ ...pathsCfg, selected_minecraft_path: p })}
                onRemove={() => {
                  const next = {
                    ...pathsCfg,
                    minecraft_paths: pathsCfg.minecraft_paths.filter((x) => x !== p),
                    selected_minecraft_path:
                      pathsCfg.selected_minecraft_path === p
                        ? pathsCfg.default_minecraft_path
                        : pathsCfg.selected_minecraft_path,
                  };
                  savePaths(next);
                }}
              />
            ))}
          {!pathsCfg.default_minecraft_path &&
            pathsCfg.minecraft_paths.length === 0 && (
              <p className="text-[10px] text-muted-foreground/60 px-1">
                点击"添加"选择游戏目录
              </p>
            )}
        </div>

        {/* Java 路径 — 路径列表 */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <Label className="text-xs text-muted-foreground">
              <Cpu className="size-3" />
              Java 路径
            </Label>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-[10px] text-muted-foreground gap-1"
              onClick={() => openDialog("java")}
            >
              <Plus className="size-3" /> 添加
            </Button>
          </div>
          {pathsCfg.java_paths.length === 0 ? (
            <p className="text-[10px] text-muted-foreground/60 px-1">
              点击"添加"选择 Java 可执行文件
            </p>
          ) : (
            pathsCfg.java_paths.map((p) => (
              <PathItem
                key={p}
                path={p}
                isSelected={pathsCfg.selected_java_path === p}
                canRemove
                onSelect={() => savePaths({ ...pathsCfg, selected_java_path: p })}
                onRemove={() => {
                  const nextPaths = pathsCfg.java_paths.filter((x) => x !== p);
                  savePaths({
                    ...pathsCfg,
                    java_paths: nextPaths,
                    selected_java_path:
                      pathsCfg.selected_java_path === p
                        ? (nextPaths[0] ?? "")
                        : pathsCfg.selected_java_path,
                  });
                }}
              />
            ))
          )}
        </div>

        {/* Wrapper 路径（可选） */}
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">
            <Package className="size-3" />
            Wrapper 路径
            <span className="text-[10px] text-muted-foreground/60 ml-1">（可选）</span>
          </Label>
          <div className="flex gap-2">
            <Input
              placeholder="留空则直接启动，无需 Wrapper"
              value={config.wrapperPath}
              onChange={(e) =>
                updateConfig({ wrapperPath: e.target.value })
              }
              className="text-xs h-8"
            />
            <Button
              variant="outline"
              size="sm"
              className="shrink-0 h-8 px-2"
              onClick={() => handleOpenFileDialog("wrapperPath")}
            >
              <FolderOpen className="size-3.5" />
            </Button>
          </div>
        </div>

        {/* 最大内存 */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <Label className="text-xs text-muted-foreground">
              最大内存 (MB)
            </Label>
            {totalMB > 0 && (
              <div className="flex items-center gap-2 text-[10px] text-muted-foreground">
                <span>系统总计 <span className="text-foreground font-medium">{totalMB >= 1024 ? `${(totalMB / 1024).toFixed(1)} GB` : `${totalMB} MB`}</span></span>
                {usedMB > 0 && (
                  <>
                    <span className="text-muted-foreground/40">·</span>
                    <span>可用 <span className="text-foreground font-medium">{((totalMB - usedMB) / 1024).toFixed(1)} GB</span></span>
                  </>
                )}
              </div>
            )}
          </div>
          {totalMB > 0 && usedMB > 0 && (() => {
            const availableMB = totalMB - usedMB;
            const percentage = Math.min(100, (Number(config.maxMemory) / availableMB) * 100);
            return (
              <div className="w-full h-1 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-primary/40 transition-all"
                  style={{ width: `${percentage}%` }}
                />
              </div>
            );
          })()}
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={1024}
              max={totalMB > 0 && usedMB > 0 ? Math.max(1024, totalMB - usedMB) : 16384}
              step={512}
              value={Number(config.maxMemory) || 4096}
              onChange={(e) => updateConfig({ maxMemory: e.target.value })}
              className="flex-1 h-1.5 accent-primary cursor-pointer"
            />
            <Input
              type="number"
              value={config.maxMemory}
              onChange={(e) => updateConfig({ maxMemory: e.target.value })}
              className="w-20 text-xs h-8 text-center"
              min={512}
              max={32768}
            />
          </div>
          <p className="text-[10px] text-muted-foreground">
            建议分配可用内存的 50%–75%
            {totalMB > 0 && usedMB > 0 && (() => {
              const availableMB = totalMB - usedMB;
              return (
                <span className="ml-1 text-muted-foreground/60">
                  （{Math.round(availableMB * 0.5)}–{Math.round(availableMB * 0.75)} MB）
                </span>
              );
            })()}
          </p>
        </div>

        {/* 版本名称 */}
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">游戏版本</Label>
          <Input
            placeholder="版本名称，如 1.21.4"
            value={config.versionName}
            onChange={(e) => updateConfig({ versionName: e.target.value })}
            className="text-xs h-8"
          />
        </div>

        {/* 加载器类型 */}
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">加载器</Label>
          <div className="flex gap-2">
            <Select
              value={config.loadType}
              onValueChange={(val) => updateConfig({ loadType: val })}
            >
              <SelectTrigger size="sm" className="flex-1 h-8 text-xs">
                <SelectValue placeholder="选择加载器类型" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="0">原版 (Vanilla)</SelectItem>
                <SelectItem value="1">Forge / Fabric / NeoForge</SelectItem>
              </SelectContent>
            </Select>
            {config.loadType !== "0" && (
              <Input
                placeholder="加载器版本目录名"
                value={config.loadName}
                onChange={(e) => updateConfig({ loadName: e.target.value })}
                className="flex-1 text-xs h-8"
              />
            )}
          </div>
        </div>

        {/* 窗口尺寸 */}
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">
            <Monitor className="size-3" />
            游戏窗口尺寸
          </Label>
          <div className="flex gap-2 items-center">
            <Input
              type="number"
              placeholder="宽度"
              value={config.windowWidth}
              onChange={(e) => updateConfig({ windowWidth: e.target.value })}
              className="text-xs h-8 w-24 text-center"
              min={1}
            />
            <span className="text-xs text-muted-foreground">×</span>
            <Input
              type="number"
              placeholder="高度"
              value={config.windowHeight}
              onChange={(e) => updateConfig({ windowHeight: e.target.value })}
              className="text-xs h-8 w-24 text-center"
              min={1}
            />
          </div>
        </div>

        {/* 玩家身份（可折叠） */}
        <details className="group">
          <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground transition-colors select-none">
            高级：玩家身份设置
          </summary>
          <div className="mt-3 space-y-3 border-l-2 border-border pl-3">
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                <User className="size-3" />
                玩家名称
              </Label>
              <Input
                placeholder="留空则使用当前账户名"
                value={config.playerName}
                onChange={(e) =>
                  updateConfig({ playerName: e.target.value })
                }
                className="text-xs h-8"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                UUID
              </Label>
              <Input
                placeholder="留空则使用账户 ID"
                value={config.uuid}
                onChange={(e) =>
                  updateConfig({ uuid: e.target.value })
                }
                className="text-xs h-8"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                认证令牌 (accessToken)
              </Label>
              <Input
                type="password"
                placeholder="可选"
                value={config.authToken}
                onChange={(e) =>
                  updateConfig({ authToken: e.target.value })
                }
                className="text-xs h-8"
              />
            </div>
          </div>
        </details>

        {/* 第三方验证（可折叠） */}
        <details className="group">
          <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground transition-colors select-none">
            高级：第三方验证设置
          </summary>
          <div className="mt-3 space-y-3 border-l-2 border-border pl-3">
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                Authlib Injector 路径
              </Label>
              <Input
                placeholder="authlib-injector.jar 路径"
                value={config.authlibInjectorPath}
                onChange={(e) =>
                  updateConfig({ authlibInjectorPath: e.target.value })
                }
                className="text-xs h-8"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                Yggdrasil API
              </Label>
              <Input
                placeholder="https://..."
                value={config.yggdrasilApi}
                onChange={(e) =>
                  updateConfig({ yggdrasilApi: e.target.value })
                }
                className="text-xs h-8"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">
                预取数据 (Base64)
              </Label>
              <Input
                placeholder="可选"
                value={config.prefetchedData}
                onChange={(e) =>
                  updateConfig({ prefetchedData: e.target.value })
                }
                className="text-xs h-8"
              />
            </div>
          </div>
        </details>
      </CardContent>
    </Card>
  );
}
