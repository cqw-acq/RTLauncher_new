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
import { FolderOpen, HardDrive, Cpu, Package, User, Monitor } from "lucide-react";

/**
 * 启动配置卡片
 * 设置 Java 路径、内存、版本等启动参数
 */
export function LaunchConfigCard() {
  const { config, updateConfig } = useLaunchContext();

  const handleOpenFileDialog = async (
    field: "minecraftPath" | "javaPath" | "wrapperPath"
  ) => {
    try {
      // 动态导入 dialog 插件，若不可用则静默跳过
      const mod = await import("@tauri-apps/plugin-dialog" as string);
      const open = mod.open;
      const isDir = field === "minecraftPath";
      const result = isDir
        ? await open({ directory: true, multiple: false })
        : await open({
            multiple: false,
            filters: [
              {
                name: "Executable",
                extensions:
                  field === "javaPath"
                    ? ["exe", ""]
                    : ["jar", "exe", ""],
              },
            ],
          });
      if (result) {
        updateConfig({ [field]: result as string });
      }
    } catch {
      // 若 dialog 插件不可用，忽略错误（用户可手动输入路径）
    }
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
        {/* Minecraft 路径 */}
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">
            <HardDrive className="size-3" />
            游戏目录
          </Label>
          <div className="flex gap-2">
            <Input
              placeholder=".minecraft 目录路径"
              value={config.minecraftPath}
              onChange={(e) =>
                updateConfig({ minecraftPath: e.target.value })
              }
              className="text-xs h-8"
            />
            <Button
              variant="outline"
              size="sm"
              className="shrink-0 h-8 px-2"
              onClick={() => handleOpenFileDialog("minecraftPath")}
            >
              <FolderOpen className="size-3.5" />
            </Button>
          </div>
        </div>

        {/* Java 路径 */}
        <div className="space-y-1.5">
          <Label className="text-xs text-muted-foreground">
            <Cpu className="size-3" />
            Java 路径
          </Label>
          <div className="flex gap-2">
            <Input
              placeholder="java 可执行文件路径"
              value={config.javaPath}
              onChange={(e) => updateConfig({ javaPath: e.target.value })}
              className="text-xs h-8"
            />
            <Button
              variant="outline"
              size="sm"
              className="shrink-0 h-8 px-2"
              onClick={() => handleOpenFileDialog("javaPath")}
            >
              <FolderOpen className="size-3.5" />
            </Button>
          </div>
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
          <Label className="text-xs text-muted-foreground">
            最大内存 (MB)
          </Label>
          <div className="flex items-center gap-3">
            <input
              type="range"
              min={1024}
              max={16384}
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
            建议分配系统内存的 50%–75%
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
