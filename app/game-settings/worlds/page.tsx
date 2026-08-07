"use client";

import React, { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { Map, ArrowLeft, Sparkles, Flame, Skull, Swords, Copy, Check } from "lucide-react";
import ResourcePanel from "@/components/resource-panel";
import { useResourcePage, ResourcePageFallback } from "@/components/resource-page-factory";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { fadeSlideUp } from "@/lib/motion";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { invoke } from "@tauri-apps/api/core";

const WORLDS_CONFIG = {
  title: "panel.worlds" as const,
  leftIcon: <Map className="size-5 text-amber-500" />,
  leftIconBg: "bg-amber-500/10",
  leftIconColor: "text-amber-500",
  instanceSubdir: "saves",
  cacheKind: "world",
  extensions: ["zip", "rar", "mcworld"],
  simplifyName: (name: string) => name.replace(/\.(zip|rar|mcworld)$/i, ""),
};

interface LevelDatInfo {
  seed: string;
  keep_inventory: boolean;
  mob_griefing: boolean;
  do_fire_tick: boolean;
  allow_commands: boolean;
}

function WorldDetailPage({
  worldName,
  minecraftPath,
  onBack,
}: {
  worldName: string;
  minecraftPath: string;
  onBack: () => void;
}) {
  const [levelInfo, setLevelInfo] = useState<LevelDatInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    const loadLevelInfo = async () => {
      try {
        const worldPath = `${minecraftPath}/saves/${worldName}`;
        const result = await invoke<LevelDatInfo>("vm_parse_level_dat", {
          worldFolderPath: worldPath,
        });
        setLevelInfo(result);
      } catch (err) {
        console.error("Failed to load level info:", err);
        setLevelInfo(null);
      } finally {
        setLoading(false);
      }
    };

    loadLevelInfo();
  }, [worldName, minecraftPath]);

  const handleRuleChange = async (paramName: string, value: boolean) => {
    if (!levelInfo) return;

    try {
      const worldPath = `${minecraftPath}/saves/${worldName}`;
      await invoke("vm_modify_game_rule", {
        worldFolderPath: worldPath,
        paramName,
        newValue: value ? "true" : "false",
      });

      setLevelInfo((prev) => {
        if (!prev) return null;
        return {
          ...prev,
          [paramName]: value,
        };
      });
    } catch (err) {
      console.error("Failed to modify game rule:", err);
    }
  };

  const handleCopySeed = async () => {
    if (!levelInfo || !levelInfo.seed) return;
    try {
      await navigator.clipboard.writeText(levelInfo.seed);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  if (loading) {
    return (
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        className="flex h-full flex-col items-center justify-center gap-3"
      >
        <div className="size-12 rounded-full bg-muted flex items-center justify-center animate-pulse">
          <Map className="size-6 text-muted-foreground" />
        </div>
        <p className="text-sm text-muted-foreground">加载存档信息...</p>
      </motion.div>
    );
  }

  return (
    <motion.div
      variants={fadeSlideUp}
      initial="initial"
      animate="animate"
      className="flex h-full flex-col"
    >
      {/* 返回按钮和标题 */}
      <div className="flex items-center gap-3 px-4 py-3 border-b">
        <Button
          variant="ghost"
          size="icon"
          className="size-8 -ml-2"
          onClick={onBack}
        >
          <ArrowLeft className="size-4" />
        </Button>
        <div>
          <h1 className="text-lg font-semibold">{worldName}</h1>
          <p className="text-xs text-muted-foreground">存档设置</p>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4 space-y-4">
        {/* 世界种子 */}
        <div className="rounded-xl border p-4">
          <div className="flex items-center gap-2 mb-3">
            <Sparkles className="size-4 text-amber-500" />
            <h2 className="text-sm font-semibold">世界种子</h2>
          </div>
          <div className="flex items-center gap-2">
            <code className="flex-1 text-sm font-mono bg-muted px-3 py-2 rounded-lg truncate">
              {levelInfo?.seed || "未设置"}
            </code>
            <Button
              variant="ghost"
              size="icon"
              className="size-8"
              onClick={handleCopySeed}
              title="复制种子"
            >
              {copied ? (
                <Check className="size-4 text-emerald-500" />
              ) : (
                <Copy className="size-4" />
              )}
            </Button>
          </div>
        </div>

        {/* 游戏规则 */}
        <div className="rounded-xl border p-4">
          <div className="flex items-center gap-2 mb-4">
            <Swords className="size-4 text-amber-500" />
            <h2 className="text-sm font-semibold">游戏规则</h2>
          </div>

          <div className="space-y-4">
            {/* 允许作弊 */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Skull className="size-4 text-red-500" />
                <div>
                  <p className="text-sm font-medium">允许作弊</p>
                  <p className="text-xs text-muted-foreground">开启后可以使用命令</p>
                </div>
              </div>
              <Switch
                checked={levelInfo?.allow_commands || false}
                onCheckedChange={(v) => handleRuleChange("allow_commands", v)}
              />
            </div>

            {/* 保持物品栏 */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Sparkles className="size-4 text-purple-500" />
                <div>
                  <p className="text-sm font-medium">保持物品栏</p>
                  <p className="text-xs text-muted-foreground">死亡后保留物品和经验</p>
                </div>
              </div>
              <Switch
                checked={levelInfo?.keep_inventory || false}
                onCheckedChange={(v) => handleRuleChange("keep_inventory", v)}
              />
            </div>

            {/* 生物破坏 */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Skull className="size-4 text-orange-500" />
                <div>
                  <p className="text-sm font-medium">生物破坏</p>
                  <p className="text-xs text-muted-foreground">生物是否可以破坏方块</p>
                </div>
              </div>
              <Switch
                checked={levelInfo?.mob_griefing || false}
                onCheckedChange={(v) => handleRuleChange("mob_griefing", v)}
              />
            </div>

            {/* 火焰蔓延 */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <Flame className="size-4 text-red-500" />
                <div>
                  <p className="text-sm font-medium">火焰蔓延</p>
                  <p className="text-xs text-muted-foreground">火焰是否会蔓延</p>
                </div>
              </div>
              <Switch
                checked={levelInfo?.do_fire_tick || false}
                onCheckedChange={(v) => handleRuleChange("do_fire_tick", v)}
              />
            </div>
          </div>
        </div>
      </div>
    </motion.div>
  );
}

export default function WorldsPage() {
  const { panel, loadingState } = useResourcePage(WORLDS_CONFIG);
  const { config, configLoaded: launchConfigLoaded } = useLaunchContext();
  const [view, setView] = useState<"list" | "detail">("list");
  const [selectedWorld, setSelectedWorld] = useState<string | null>(null);

  // 准备中状态：configLoaded=true 但 instances 还在扫描 / minecraftPath 待定
  const [preparingTimeout, setPreparingTimeout] = useState(false);
  useEffect(() => {
    if (!loadingState.configLoaded) return;
    const t = window.setTimeout(() => setPreparingTimeout(true), 1200);
    return () => window.clearTimeout(t);
  }, [loadingState.configLoaded]);

  if (!loadingState.configLoaded || (loadingState.configLoaded && !preparingTimeout && (loadingState.instancesScanning || !loadingState.minecraftPath))) {
    return (
      <ResourcePageFallback
        title="pageFactory.loadingConfiguration"
        subtitle="pageFactory.pleaseWait"
      />
    );
  }

  if (!loadingState.minecraftPath) {
    return (
      <ResourcePageFallback
        title="pageFactory.gameDirectoryIsNotConfigured"
        subtitle="pageFactory.configureTheMinecraftGameDirectoryOnTheLaunchPage"
      />
    );
  }

  // 检查路径是否为有效的绝对路径
  const isValidAbsolutePath = (path: string | undefined): boolean => {
    if (!path) return false;
    const trimmed = path.trim();
    if (trimmed.length === 0) return false;
    
    // Windows 绝对路径格式: C:\... 或 C:/...
    const isWindows = trimmed.length >= 3 && 
                      trimmed[1] === ':' && 
                      (trimmed[2] === '\\' || trimmed[2] === '/');
    // Windows UNC 路径: \\server\share
    const isUNC = trimmed.startsWith('\\\\') || trimmed.startsWith('//');
    // Unix 绝对路径格式: /...
    const isUnix = trimmed.startsWith('/');
    
    return isWindows || isUNC || isUnix;
  };

  // 检查路径是否为有效的游戏目录路径
  const isValidMinecraftPath = (path: string | undefined): boolean => {
    // 简化验证：只检查是否是有效的绝对路径
    // 用户可能使用自定义路径，不一定包含 .minecraft
    return isValidAbsolutePath(path);
  };

  // 详情子页面
  if (view === "detail" && selectedWorld) {
    // 使用 instanceDir（包含 versions/<instance> 路径）作为存档目录的基础路径
    const effectiveInstanceDir = loadingState.instanceDir || config.minecraftPath;
    
    if (!effectiveInstanceDir || !isValidMinecraftPath(effectiveInstanceDir)) {
      return (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex h-full flex-col items-center justify-center gap-3 p-4"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">
            <Map className="size-6 text-muted-foreground" />
          </div>
          <p className="text-sm font-medium">未配置游戏目录</p>
          <p className="text-xs text-muted-foreground">请先在「启动」页面配置 Minecraft 游戏目录</p>
          <Button onClick={() => {
            setSelectedWorld(null);
            setView("list");
          }}>
            返回
          </Button>
        </motion.div>
      );
    }
    
    return (
      <WorldDetailPage
        worldName={selectedWorld}
        minecraftPath={effectiveInstanceDir}
        onBack={() => {
          setSelectedWorld(null);
          setView("list");
        }}
      />
    );
  }

  // 列表主页面
  const handleOpenDetail = (worldName: string) => {
    setSelectedWorld(worldName);
    setView("detail");
  };

  return (
    <ResourcePanel
      {...panel}
      onOpenModDetail={(fileName) => handleOpenDetail(fileName.replace(/\.(zip|rar|mcworld)$/i, ""))}
    />
  );
}