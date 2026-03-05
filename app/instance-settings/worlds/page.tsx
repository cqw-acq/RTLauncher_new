"use client";

import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Globe, Search, RefreshCw, Copy, Map, Folder,
  ChevronRight, Dices, ShieldCheck, Flame, Wand2, Terminal, ArrowLeft, Loader2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { fadeSlideUp, staggerContainer, staggerItem } from "@/lib/motion";
import { useInstancePath } from "@/hooks/use-instance-path";
import { useDirFiles } from "@/hooks/use-dir-files";
import { useWorldInfo } from "@/hooks/use-world-info";
import type { DirEntry } from "@/types";

/* ------------------------------------------------------------------ */
/* 单条游戏规则开关行                                                     */
/* ------------------------------------------------------------------ */
function RuleRow({
  icon: Icon,
  label,
  description,
  value,
  disabled,
  onToggle,
}: {
  icon: React.ElementType;
  label: string;
  description: string;
  value: boolean;
  disabled: boolean;
  onToggle: () => void;
}) {
  return (
    <div className="flex items-center gap-3 py-2.5">
      <div className="size-8 rounded-lg bg-amber-50 dark:bg-amber-900/20 flex items-center justify-center shrink-0">
        <Icon className="size-4 text-amber-600 dark:text-amber-400" />
      </div>
      <div className="flex-1 min-w-0">
        <p className="text-sm font-medium">{label}</p>
        <p className="text-xs text-muted-foreground">{description}</p>
      </div>
      <button
        role="switch"
        aria-checked={value}
        disabled={disabled}
        onClick={onToggle}
        className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors focus:outline-none focus:ring-2 focus:ring-amber-500 focus:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 ${
          value ? "bg-amber-500" : "bg-input"
        }`}
      >
        <span
          className={`pointer-events-none inline-block size-4 rounded-full bg-white shadow-lg transition-transform ${
            value ? "translate-x-4" : "translate-x-0"
          }`}
        />
      </button>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* 存档设置面板                                                           */
/* ------------------------------------------------------------------ */
function WorldSettingsPanel({
  world,
  savesDir,
  onBack,
}: {
  world: DirEntry;
  savesDir: string;
  onBack: () => void;
}) {
  const worldPath = `${savesDir}/${world.name}`;
  const { info, loading, error, refetch, modifyGameRule } = useWorldInfo(worldPath);
  const [saving, setSaving] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const handleToggle = async (
    key: "keepInventory" | "mobGriefing" | "doFireTick" | "allowCommands",
    current: boolean
  ) => {
    setSaving(key);
    setSaveError(null);
    try {
      await modifyGameRule(key, !current);
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(null);
    }
  };

  return (
    <motion.div
      key="settings"
      initial={{ opacity: 0, x: 24 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 24 }}
      transition={{ duration: 0.18, ease: "easeOut" }}
      className="flex flex-col h-full"
    >
      {/* 顶栏 */}
      <div className="flex items-center gap-2 shrink-0 mb-4">
        <Button variant="ghost" size="icon" className="size-8" onClick={onBack}>
          <ArrowLeft className="size-4" />
        </Button>
        <div className="size-8 rounded-lg bg-amber-50 dark:bg-amber-900/20 flex items-center justify-center">
          <Folder className="size-4 text-amber-600 dark:text-amber-400" />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold truncate">{world.name}</p>
          <p className="text-xs text-muted-foreground">存档设置</p>
        </div>
        <Button variant="ghost" size="icon" className="size-8" onClick={refetch} disabled={loading}>
          <RefreshCw className={`size-3.5 ${loading ? "animate-spin" : ""}`} />
        </Button>
      </div>

      <Separator className="shrink-0 mb-4" />

      {loading ? (
        <div className="flex flex-col items-center justify-center flex-1 gap-2">
          <Loader2 className="size-5 text-muted-foreground animate-spin" />
          <p className="text-xs text-muted-foreground">解析 level.dat…</p>
        </div>
      ) : error ? (
        <div className="flex flex-col items-center justify-center flex-1 gap-2 text-center">
          <p className="text-sm text-destructive">读取失败</p>
          <p className="text-xs text-muted-foreground">{error}</p>
          <Button variant="outline" size="sm" onClick={refetch}>重试</Button>
        </div>
      ) : !info ? null : (
        <div className="flex-1 overflow-y-auto space-y-4">
          {/* 种子（只读） */}
          <div>
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">基本信息</p>
            <div className="rounded-xl border bg-card p-3 flex items-center gap-3">
              <div className="size-8 rounded-lg bg-amber-50 dark:bg-amber-900/20 flex items-center justify-center shrink-0">
                <Dices className="size-4 text-amber-600 dark:text-amber-400" />
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium">世界种子</p>
                <p className="text-xs text-muted-foreground">只读 · 无法修改</p>
              </div>
              <Badge variant="secondary" className="font-mono text-xs max-w-[120px] truncate">
                {info.seed}
              </Badge>
            </div>
          </div>

          {/* 游戏规则 */}
          <div>
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide mb-2">游戏规则</p>
            <div className="rounded-xl border bg-card px-3 divide-y">
              <RuleRow
                icon={ShieldCheck}
                label="保留物品栏"
                description="死亡后不掉落物品"
                value={info.keep_inventory}
                disabled={saving !== null}
                onToggle={() => handleToggle("keepInventory", info.keep_inventory)}
              />
              <RuleRow
                icon={Globe}
                label="生物破坏地形"
                description="苦力怕爆炸等会改变地形"
                value={info.mob_griefing}
                disabled={saving !== null}
                onToggle={() => handleToggle("mobGriefing", info.mob_griefing)}
              />
              <RuleRow
                icon={Flame}
                label="火焰蔓延"
                description="火焰随时间自然蔓延"
                value={info.do_fire_tick}
                disabled={saving !== null}
                onToggle={() => handleToggle("doFireTick", info.do_fire_tick)}
              />
              <RuleRow
                icon={Terminal}
                label="允许指令"
                description="在世界中使用命令方块/作弊指令"
                value={info.allow_commands}
                disabled={saving !== null}
                onToggle={() => handleToggle("allowCommands", info.allow_commands)}
              />
            </div>
            {saving && (
              <p className="text-xs text-muted-foreground mt-2 flex items-center gap-1">
                <Loader2 className="size-3 animate-spin" /> 正在保存…
              </p>
            )}
            {saveError && (
              <p className="text-xs text-destructive mt-2">
                保存失败：{saveError}
              </p>
            )}
          </div>
        </div>
      )}
    </motion.div>
  );
}

/* ------------------------------------------------------------------ */
/* 主页面                                                                */
/* ------------------------------------------------------------------ */
export default function WorldsPage() {
  const [search, setSearch] = useState("");
  const [selectedWorld, setSelectedWorld] = useState<DirEntry | null>(null);
  const { instanceDir, selectedInstance, minecraftPath } = useInstancePath();
  const savesDir = instanceDir ? `${instanceDir}/saves` : undefined;
  const { entries, loading, error, refetch } = useDirFiles(savesDir);

  const worlds = entries.filter((e) => e.is_dir);
  const filtered = worlds.filter((e) =>
    e.name.toLowerCase().includes(search.toLowerCase())
  );

  const copyPath = () => {
    if (savesDir) navigator.clipboard.writeText(savesDir);
  };

  return (
    <div className="flex h-full flex-col gap-4 p-4 overflow-y-auto">
      <AnimatePresence mode="wait">
        {selectedWorld && savesDir ? (
          <WorldSettingsPanel
            key={selectedWorld.name}
            world={selectedWorld}
            savesDir={savesDir}
            onBack={() => setSelectedWorld(null)}
          />
        ) : (
          <motion.div
            key="list"
            initial={{ opacity: 0, x: -24 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -24 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="flex flex-col gap-4"
          >
            {/* 标题 */}
            <div className="flex items-center gap-3 shrink-0">
              <div className="flex size-9 items-center justify-center rounded-xl bg-amber-500/10">
                <Globe className="size-5 text-amber-500" />
              </div>
              <div>
                <h1 className="text-lg font-semibold leading-none">世界/存档</h1>
                <p className="mt-1 text-xs text-muted-foreground">
                  {selectedInstance
                    ? `${selectedInstance.name} · ${worlds.length} 个存档`
                    : "请先在启动配置中设置游戏目录"}
                </p>
              </div>
              <div className="ml-auto flex items-center gap-2">
                <Badge variant="secondary" className="text-xs">{worlds.length} 个</Badge>
                <Button variant="ghost" size="icon" className="size-8" onClick={refetch} disabled={loading}>
                  <RefreshCw className={`size-3.5 ${loading ? "animate-spin" : ""}`} />
                </Button>
                <Button variant="ghost" size="icon" className="size-8" onClick={copyPath} title={savesDir ?? ""}>
                  <Copy className="size-3.5" />
                </Button>
              </div>
            </div>

            {/* 搜索 */}
            <div className="relative shrink-0">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
              <Input
                placeholder="搜索存档..."
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="pl-8 h-8 text-sm"
              />
            </div>

            <Separator />

            {/* 内容 */}
            {!minecraftPath ? (
              <motion.div variants={fadeSlideUp} initial="initial" animate="animate"
                className="flex flex-col items-center justify-center flex-1 gap-3 text-center py-12"
              >
                <div className="size-12 rounded-full bg-muted flex items-center justify-center">
                  <Globe className="size-6 text-muted-foreground" />
                </div>
                <p className="text-sm font-medium">未配置游戏目录</p>
                <p className="text-xs text-muted-foreground">请先在「启动」页面配置游戏目录路径</p>
              </motion.div>
            ) : loading ? (
              <div className="space-y-2">
                {[...Array(4)].map((_, i) => (
                  <div key={i} className="h-14 rounded-lg bg-muted/50 animate-pulse" />
                ))}
              </div>
            ) : error ? (
              <motion.div variants={fadeSlideUp} initial="initial" animate="animate"
                className="flex flex-col items-center justify-center flex-1 gap-2 text-center"
              >
                <p className="text-sm text-destructive">读取失败</p>
                <p className="text-xs text-muted-foreground">{error}</p>
                <Button variant="outline" size="sm" onClick={refetch}>重试</Button>
              </motion.div>
            ) : filtered.length === 0 ? (
              <motion.div variants={fadeSlideUp} initial="initial" animate="animate"
                className="flex flex-col items-center justify-center flex-1 gap-3 text-center py-12"
              >
                <div className="size-12 rounded-full bg-muted flex items-center justify-center">
                  <Map className="size-6 text-muted-foreground" />
                </div>
                <p className="text-sm font-medium">{search ? "没有匹配的存档" : "saves 目录为空"}</p>
                <p className="text-xs text-muted-foreground">
                  {search ? "尝试更换搜索关键词" : "游戏世界将保存到 saves 文件夹"}
                </p>
              </motion.div>
            ) : (
              <motion.div variants={staggerContainer} initial="initial" animate="animate" className="space-y-1">
                {filtered.map((world) => (
                  <motion.div key={world.name} variants={staggerItem}>
                    <button
                      onClick={() => setSelectedWorld(world)}
                      className="w-full flex items-center gap-3 px-3 py-3 rounded-lg hover:bg-accent/50 transition-colors group text-left"
                    >
                      <div className="size-9 rounded-lg bg-amber-50 dark:bg-amber-900/20 flex items-center justify-center shrink-0">
                        <Folder className="size-4 text-amber-600 dark:text-amber-400" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className="text-sm font-medium truncate">{world.name}</p>
                        <p className="text-xs text-muted-foreground">点击查看设置</p>
                      </div>
                      <ChevronRight className="size-4 text-muted-foreground shrink-0 opacity-0 group-hover:opacity-100 transition-opacity" />
                    </button>
                  </motion.div>
                ))}
              </motion.div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
