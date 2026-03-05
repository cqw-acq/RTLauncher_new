"use client";

import { motion } from "framer-motion";
import Link from "next/link";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { INSTANCE_CARDS } from "@/constants/data";
import { cn } from "@/lib/utils";
import { staggerContainer, staggerItem } from "@/lib/motion";
import { useInstancePath } from "@/hooks/use-instance-path";
import { useResourcePacks } from "@/hooks/use-resource-packs";
import { useDirFiles } from "@/hooks/use-dir-files";

export function InstanceCardGrid() {
  const { instanceDir, selectedInstance } = useInstancePath();

  // mods count 来自 Rust 扫描结果
  const modsCount = selectedInstance?.mods_count;

  // 世界（saves/ 下的目录数）
  const { entries: worldEntries } = useDirFiles(
    instanceDir ? `${instanceDir}/saves` : undefined
  );
  const worldCount = worldEntries.filter((e) => e.is_dir).length;
  const latestWorld = worldEntries.find((e) => e.is_dir)?.name;

  // 资源包
  const { packs: resourcePacks } = useResourcePacks(instanceDir ?? undefined);

  // 光影包（shaderpacks/）
  const { entries: shaderEntries } = useDirFiles(
    instanceDir ? `${instanceDir}/shaderpacks` : undefined
  );

  // 截图
  const { entries: screenshotEntries } = useDirFiles(
    instanceDir ? `${instanceDir}/screenshots` : undefined,
    ["png", "jpg", "jpeg", "webp"]
  );

  // 投影原理图
  const { entries: schematicEntries } = useDirFiles(
    instanceDir ? `${instanceDir}/schematics` : undefined,
    ["schematic", "nbt", "litematic", "schem"]
  );

  /** 根据卡片 id 生成动态 stats，无数据时回退到 baseStats */
  const getDynamicStats = (cardId: string, baseStats: string[]): string[] => {
    switch (cardId) {
      case "mods":
        if (modsCount != null)
          return [`• 已安装：${modsCount} 个模组`, ...baseStats.slice(1)];
        break;
      case "worlds":
        if (instanceDir) {
          const countStr = worldCount > 0 ? `${worldCount} 个` : "0 个";
          const recent = latestWorld ? `• 最近游戏：${latestWorld}` : baseStats[1];
          return [`• 游戏存档：${countStr}`, recent, baseStats[2]];
        }
        break;
      case "resources":
        if (instanceDir) {
          const first = resourcePacks[0]?.name;
          const current = first ? `• 当前使用：${first}` : baseStats[0];
          return [current, `• 已安装：${resourcePacks.length} 个包`, baseStats[2]];
        }
        break;
      case "shaders":
        if (instanceDir) {
          const firstName = shaderEntries[0]?.name.replace(/\.[^.]+$/, "");
          const current = firstName ? `• 当前光影：${firstName}` : baseStats[0];
          return [current, `• 已安装：${shaderEntries.length} 个`, baseStats[2]];
        }
        break;
      case "screenshots":
        if (instanceDir)
          return [
            `• 总数：${screenshotEntries.length} 张`,
            screenshotEntries.length > 0 ? baseStats[1] : "• 上次截图：从不",
            baseStats[2],
          ];
        break;
      case "schematics":
        if (instanceDir) {
          const latest = schematicEntries[0]?.name.replace(/\.[^.]+$/, "");
          const recentStr = latest ? `• 最近使用：${latest}` : baseStats[1];
          return [`• 原理图：${schematicEntries.length} 个`, recentStr, baseStats[2]];
        }
        break;
    }
    return baseStats;
  };

  return (
    <motion.div
      className="flex-1 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 auto-rows-[minmax(0,1fr)] h-full items-stretch min-h-0"
      variants={staggerContainer}
      initial="initial"
      animate="animate"
    >
      {INSTANCE_CARDS.map((card) => (
        <motion.div key={card.id} variants={staggerItem} className="h-full">
          <Link href={card.href} className="block h-full">
            <Card className="shadow-sm hover:shadow-xl transition-all duration-300 cursor-pointer h-full flex flex-col border hover:border-primary/40">
              <CardHeader>
                {/* 图标 */}
                <div
                  className={cn(
                    "w-14 h-14 rounded-xl flex items-center justify-center mb-4",
                    card.iconBgColor
                  )}
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    className={cn("h-7 w-7", card.iconColor)}
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                  >
                    {card.icon}
                  </svg>
                </div>
                <CardTitle className="text-lg">{card.title}</CardTitle>
                <CardDescription className="text-xs">{card.description}</CardDescription>
              </CardHeader>
              <CardContent className="flex-1 flex flex-col justify-between">
                <div className="space-y-2">
                  {getDynamicStats(card.id, card.stats).map((stat, index) => (
                    <p key={index} className="text-xs text-muted-foreground">
                      {stat}
                    </p>
                  ))}
                </div>
              </CardContent>
            </Card>
          </Link>
        </motion.div>
      ))}
    </motion.div>
  );
}
