"use client";

import { motion } from "framer-motion";
import Link from "next/link";
import { useMemo } from "react";
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
import { useResourcePacks } from "@/hooks/use-resource-packs";
import { useDirFiles } from "@/hooks/use-dir-files";
import type { InstanceData } from "@/types";
import { useI18n, type Translation } from "@/components/i18n/use-i18n";

type InstanceCardGridProps = {
  instanceDir: string | undefined;
  selectedInstance: InstanceData | null;
};

const CARD_COPY: Record<string, { title: Translation; description: Translation; stats: Translation[] }> = {
  mods: {
    title: "home.cardGrid.mods",
    description: "home.cardGrid.manageYourMods",
    stats: ["home.cardGrid.installed72Mods", "home.cardGrid.updatesAvailable3", "home.cardGrid.editConfigurationFiles"],
  },
  worlds: {
    title: "home.cardGrid.worlds",
    description: "home.cardGrid.manageWorldSaves",
    stats: ["home.cardGrid.worldSaves6", "home.cardGrid.recentlyPlayedRtlWorld", "home.cardGrid.automaticBackups"],
  },
  resources: {
    title: "home.cardGrid.resourcePacks",
    description: "home.cardGrid.manageGameTextures",
    stats: ["home.cardGrid.currentDefaultHd", "home.cardGrid.installed4Packs", "home.cardGrid.resourcePackOrder"],
  },
  shaders: {
    title: "home.cardGrid.shaders",
    description: "home.cardGrid.enhancedVisualEffects",
    stats: ["home.cardGrid.currentShaderBsl", "home.cardGrid.installed3", "home.cardGrid.performanceSettings"],
  },
  screenshots: {
    title: "home.cardGrid.screenshots",
    description: "home.cardGrid.manageGameScreenshots",
    stats: ["home.cardGrid.total126", "home.cardGrid.latestScreenshotToday", "home.cardGrid.quickSharing"],
  },
  schematics: {
    title: "home.cardGrid.schematics",
    description: "home.cardGrid.manageBuildingDesigns",
    stats: ["home.cardGrid.schematics12", "home.cardGrid.recentlyUsedRedstoneCastle", "home.cardGrid.quickDeployment"],
  },
};

export function InstanceCardGrid({
  instanceDir,
  selectedInstance,
}: InstanceCardGridProps) {
  const { t } = useI18n();
  // mods count 来自 Rust 扫描结果
  const modsCount = selectedInstance?.mods_count;

  // 使用 useMemo 稳定化路径字符串，避免每次渲染都重新创建
  const savesPath = useMemo(() => (instanceDir ? `${instanceDir}/saves` : undefined), [instanceDir]);
  const shaderpacksPath = useMemo(() => (instanceDir ? `${instanceDir}/shaderpacks` : undefined), [instanceDir]);
  const screenshotsPath = useMemo(() => (instanceDir ? `${instanceDir}/screenshots` : undefined), [instanceDir]);
  const schematicsPath = useMemo(() => (instanceDir ? `${instanceDir}/schematics` : undefined), [instanceDir]);

  // 世界（saves/ 下的目录数）
  const { entries: worldEntries } = useDirFiles(savesPath);
  const worldCount = worldEntries.filter((e) => e.is_dir).length;
  const latestWorld = worldEntries.find((e) => e.is_dir)?.name;

  // 资源包
  const { packs: resourcePacks } = useResourcePacks(instanceDir ?? undefined);

  // 光影包（shaderpacks/）
  const { entries: shaderEntries } = useDirFiles(shaderpacksPath);

  // 截图
  const { entries: screenshotEntries } = useDirFiles(
    screenshotsPath,
    ["png", "jpg", "jpeg", "webp"]
  );

  // 投影原理图
  const { entries: schematicEntries } = useDirFiles(
    schematicsPath,
    ["schematic", "nbt", "litematic", "schem"]
  );

  /** 根据卡片 id 生成动态 stats，无数据时回退到 baseStats */
  const getDynamicStats = (cardId: string, baseStats: string[]): string[] => {
    switch (cardId) {
      case "mods":
        if (modsCount != null)
          return [t("home.cardGrid.installedModsCountMods", { modsCount: modsCount }), ...baseStats.slice(1)];
        break;
      case "worlds":
        if (instanceDir) {
          const countStr = `${worldCount}`;
          const recent = latestWorld ? t("home.cardGrid.recentlyPlayedLatestWorld", { latestWorld: latestWorld }) : baseStats[1];
          return [t("home.cardGrid.worldSavesCountStr", { countStr: countStr }), recent, baseStats[2]];
        }
        break;
      case "resources":
        if (instanceDir) {
          const first = resourcePacks[0]?.name;
          const current = first ? t("home.cardGrid.currentFirst", { first: first }) : baseStats[0];
          return [current, t("home.cardGrid.installedLengthPacks", { length: resourcePacks.length }), baseStats[2]];
        }
        break;
      case "shaders":
        if (instanceDir) {
          const firstName = shaderEntries[0]?.name.replace(/\.[^.]+$/, "");
          const current = firstName ? t("home.cardGrid.currentShaderFirstName", { firstName: firstName }) : baseStats[0];
          return [current, t("home.cardGrid.installedLength", { length: shaderEntries.length }), baseStats[2]];
        }
        break;
      case "screenshots":
        if (instanceDir)
          return [
            t("home.cardGrid.totalLength", { length: screenshotEntries.length }),
            screenshotEntries.length > 0 ? baseStats[1] : t("home.cardGrid.lastScreenshotNever"),
            baseStats[2],
          ];
        break;
      case "schematics":
        if (instanceDir) {
          const latest = schematicEntries[0]?.name.replace(/\.[^.]+$/, "");
          const recentStr = latest ? t("home.cardGrid.recentlyUsedLatest", { latest: latest }) : baseStats[1];
          return [t("home.cardGrid.schematicsLength", { length: schematicEntries.length }), recentStr, baseStats[2]];
        }
        break;
    }
    return baseStats;
  };

  return (
    <motion.div
      className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3"
      variants={staggerContainer}
      initial="initial"
      animate="animate"
    >
      {INSTANCE_CARDS.map((card) => {
        const copy = CARD_COPY[card.id];
        return (
        <motion.div key={card.id} variants={staggerItem} className="h-full">
          <Link
            href={card.href}
            prefetch={false}
            className="block h-full"
            suppressHydrationWarning
          >
            <Card className="shadow-sm hover:shadow-xl transition-all duration-300 cursor-pointer h-full flex flex-col border hover:border-primary/40">
              <CardHeader>
                {/* 图标 */}
                <div
                  className={cn(
                    "mb-3 flex size-11 items-center justify-center rounded-xl",
                    card.iconBgColor
                  )}
                >
                  <svg
                    xmlns="http://www.w3.org/2000/svg"
                    className={cn("size-5", card.iconColor)}
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                  >
                    {card.icon}
                  </svg>
                </div>
                <CardTitle>{t(copy.title)}</CardTitle>
                <CardDescription className="text-xs">{t(copy.description)}</CardDescription>
              </CardHeader>
              <CardContent className="px-4">
                <div className="space-y-1.5">
                  {getDynamicStats(card.id, copy.stats.map((stat) => t(stat))).map((stat, index) => (
                    <p key={index} className="text-xs text-muted-foreground">
                      {stat}
                    </p>
                  ))}
                </div>
              </CardContent>
            </Card>
          </Link>
        </motion.div>
      )})}
    </motion.div>
  );
}
