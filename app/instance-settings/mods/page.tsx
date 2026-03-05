"use client";

import { useState } from "react";
import { motion } from "framer-motion";
import { Package, Search, RefreshCw, Copy, FolderOpen, Puzzle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { fadeSlideUp, staggerContainer, staggerItem } from "@/lib/motion";
import { useInstancePath } from "@/hooks/use-instance-path";
import { useDirFiles } from "@/hooks/use-dir-files";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export default function ModsPage() {
  const [search, setSearch] = useState("");
  const { instanceDir, selectedInstance, minecraftPath } = useInstancePath();
  const modsDir = instanceDir ? `${instanceDir}/mods` : undefined;
  const { entries, loading, error, refetch } = useDirFiles(modsDir, ["jar"]);

  const filtered = entries.filter((e) =>
    e.name.toLowerCase().includes(search.toLowerCase())
  );

  const copyPath = () => {
    if (modsDir) navigator.clipboard.writeText(modsDir);
  };

  return (
    <div className="flex h-full flex-col gap-4 p-4 overflow-y-auto">
      {/* 标题 */}
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        className="flex items-center gap-3 shrink-0"
      >
        <div className="flex size-9 items-center justify-center rounded-xl bg-emerald-500/10">
          <Package className="size-5 text-emerald-500" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none">Mods 管理</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            {selectedInstance
              ? `${selectedInstance.name} · ${entries.length} 个模组`
              : "请先在启动配置中设置游戏目录"}
          </p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Badge variant="secondary" className="text-xs">
            {entries.length} 个
          </Badge>
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            onClick={refetch}
            disabled={loading}
          >
            <RefreshCw className={`size-3.5 ${loading ? "animate-spin" : ""}`} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            onClick={copyPath}
            title={modsDir ?? ""}
          >
            <Copy className="size-3.5" />
          </Button>
        </div>
      </motion.div>

      {/* 搜索栏 */}
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        transition={{ delay: 0.05 }}
        className="shrink-0"
      >
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
          <Input
            placeholder="搜索模组..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8 h-8 text-sm"
          />
        </div>
      </motion.div>

      <Separator />

      {/* 内容区 */}
      {!minecraftPath ? (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center flex-1 gap-3 text-center"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">
            <Package className="size-6 text-muted-foreground" />
          </div>
          <p className="text-sm font-medium">未配置游戏目录</p>
          <p className="text-xs text-muted-foreground">
            请先在「启动」页面配置游戏目录路径
          </p>
        </motion.div>
      ) : loading ? (
        <div className="space-y-2">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="h-12 rounded-lg bg-muted/50 animate-pulse" />
          ))}
        </div>
      ) : error ? (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center flex-1 gap-2 text-center"
        >
          <p className="text-sm text-destructive">读取失败</p>
          <p className="text-xs text-muted-foreground">{error}</p>
          <Button variant="outline" size="sm" onClick={refetch}>
            重试
          </Button>
        </motion.div>
      ) : filtered.length === 0 ? (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center flex-1 gap-3 text-center"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">
            <Puzzle className="size-6 text-muted-foreground" />
          </div>
          <p className="text-sm font-medium">
            {search ? "没有匹配的模组" : "mods 目录为空"}
          </p>
          <p className="text-xs text-muted-foreground">
            {search
              ? "尝试更换搜索关键词"
              : "将 .jar 格式的模组文件放入 mods 文件夹"}
          </p>
        </motion.div>
      ) : (
        <motion.div
          variants={staggerContainer}
          initial="initial"
          animate="animate"
          className="space-y-1"
        >
          {filtered.map((entry) => {
            const displayName = entry.name.replace(/\.jar$/i, "");
            return (
              <motion.div
                key={entry.name}
                variants={staggerItem}
                className="flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-accent/50 transition-colors group"
              >
                <div className="size-8 rounded-lg bg-emerald-50 dark:bg-emerald-900/20 flex items-center justify-center shrink-0">
                  <Puzzle className="size-4 text-emerald-600 dark:text-emerald-400" />
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium truncate">{displayName}</p>
                  <p className="text-xs text-muted-foreground">.jar</p>
                </div>
                <Badge variant="outline" className="text-xs shrink-0">
                  {formatBytes(entry.size)}
                </Badge>
              </motion.div>
            );
          })}
        </motion.div>
      )}
    </div>
  );
}
