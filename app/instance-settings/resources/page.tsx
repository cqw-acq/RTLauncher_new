"use client";

import { useState } from "react";
import { motion } from "framer-motion";
import { Image, Search, RefreshCw, Copy, ImageOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Separator } from "@/components/ui/separator";
import { fadeSlideUp, staggerContainer, staggerItem } from "@/lib/motion";
import { useInstancePath } from "@/hooks/use-instance-path";
import { useResourcePacks } from "@/hooks/use-resource-packs";

export default function ResourcesPage() {
  const [search, setSearch] = useState("");
  const { instanceDir, selectedInstance, minecraftPath } = useInstancePath();
  const { packs, loading, error, refetch } = useResourcePacks(instanceDir);

  const filtered = packs.filter((p) =>
    p.name.toLowerCase().includes(search.toLowerCase())
  );

  const copyPath = () => {
    if (instanceDir) navigator.clipboard.writeText(`${instanceDir}/resourcepacks`);
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
        <div className="flex size-9 items-center justify-center rounded-xl bg-blue-500/10">
          <Image className="size-5 text-blue-500" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none">资源包</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            {selectedInstance
              ? `${selectedInstance.name} · ${packs.length} 个资源包`
              : "请先在启动配置中设置游戏目录"}
          </p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Badge variant="secondary" className="text-xs">
            {packs.length} 个
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
            title={instanceDir ? `${instanceDir}/resourcepacks` : ""}
          >
            <Copy className="size-3.5" />
          </Button>
        </div>
      </motion.div>

      {/* 搜索 */}
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
            placeholder="搜索资源包..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8 h-8 text-sm"
          />
        </div>
      </motion.div>

      <Separator />

      {/* 内容 */}
      {!minecraftPath ? (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center flex-1 gap-3 text-center"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">
            <Image className="size-6 text-muted-foreground" />
          </div>
          <p className="text-sm font-medium">未配置游戏目录</p>
          <p className="text-xs text-muted-foreground">
            请先在「启动」页面配置游戏目录路径
          </p>
        </motion.div>
      ) : loading ? (
        <div className="space-y-2">
          {[...Array(4)].map((_, i) => (
            <div key={i} className="h-14 rounded-lg bg-muted/50 animate-pulse" />
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
          <Button variant="outline" size="sm" onClick={refetch}>重试</Button>
        </motion.div>
      ) : filtered.length === 0 ? (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center flex-1 gap-3 text-center"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">
            <ImageOff className="size-6 text-muted-foreground" />
          </div>
          <p className="text-sm font-medium">
            {search ? "没有匹配的资源包" : "resourcepacks 目录为空"}
          </p>
          <p className="text-xs text-muted-foreground">
            {search
              ? "尝试更换搜索关键词"
              : "将资源包（文件夹格式）放入 resourcepacks 文件夹"}
          </p>
        </motion.div>
      ) : (
        <motion.div
          variants={staggerContainer}
          initial="initial"
          animate="animate"
          className="space-y-1"
        >
          {filtered.map((pack) => (
            <motion.div
              key={pack.name}
              variants={staggerItem}
              className="flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-accent/50 transition-colors"
            >
              {/* 图标 */}
              <div className="size-10 rounded-lg bg-blue-50 dark:bg-blue-900/20 flex items-center justify-center shrink-0 overflow-hidden border">
                {pack.icon_path ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={`asset://localhost/${pack.icon_path.replace(/^\//, "")}`}
                    alt={pack.name}
                    className="size-full object-cover"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = "none";
                    }}
                  />
                ) : (
                  <Image className="size-4 text-blue-500" />
                )}
              </div>
              <div className="flex-1 min-w-0">
                <p className="text-sm font-medium truncate">{pack.name}</p>
                {pack.mc_version_range && (
                  <p className="text-xs text-muted-foreground truncate">
                    {pack.mc_version_range}
                  </p>
                )}
              </div>
              {pack.mc_version_range && (
                <Badge variant="secondary" className="text-xs shrink-0">
                  {pack.mc_version_range}
                </Badge>
              )}
            </motion.div>
          ))}
        </motion.div>
      )}
    </div>
  );
}
