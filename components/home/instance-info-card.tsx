"use client";

import { motion } from "framer-motion";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  FolderOpen,
  ImageIcon,
  MessageSquare,
  BadgeCheck,
  Smile,
  Sparkles,
} from "lucide-react";
import { Cpu } from "lucide-react";
import { fadeSlideUp } from "@/lib/motion";
import type { InstanceData } from "@/types";
import { useInstancePath } from "@/hooks/use-instance-path";
import { useDirFiles } from "@/hooks/use-dir-files";
import { useResourcePacks } from "@/hooks/use-resource-packs";


interface InfoItemProps {
  label: string;
  value: string;
  tooltip: string;
  icon: React.ReactNode;
}

function InfoItem({ label, value, tooltip, icon }: InfoItemProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <div className="group/item p-3 rounded-xl bg-card hover:bg-accent transition-all duration-200 cursor-help border shadow-sm hover:shadow-md">
          <div className="flex items-center gap-3">
            <div className="h-8 w-8 flex items-center justify-center rounded-lg bg-muted group-hover/item:bg-accent transition-colors duration-200">
              <div className="size-4 text-muted-foreground">{icon}</div>
            </div>
            <div className="min-w-0 flex-1">
              <p className="text-xs text-muted-foreground truncate">{label}</p>
              <p className="text-sm font-semibold truncate">{value}</p>
            </div>
          </div>
        </div>
      </TooltipTrigger>
      <TooltipContent side="right" className="max-w-xs">
        <p className="text-sm">{tooltip}</p>
      </TooltipContent>
    </Tooltip>
  );
}

export function InstanceInfoCard({ instance }: { instance?: InstanceData | null }) {
  const mcVersion = instance?.minecraft_version ?? "—";
  const loader = instance?.loader ?? "—";
  const modsCount = instance != null ? String(instance.mods_count) : "—";

  // 存档数量 & 资源包数量
  const { instanceDir } = useInstancePath();
  const { entries: worldEntries } = useDirFiles(
    instanceDir ? `${instanceDir}/saves` : undefined
  );
  const worldsCount = instance != null
    ? String(worldEntries.filter((e) => e.is_dir).length)
    : "—";
  const { packs: resourcePacks } = useResourcePacks(instanceDir ?? undefined);
  const resourcePacksCount = instance != null ? String(resourcePacks.length) : "—";

  const { entries: shaderEntries } = useDirFiles(
    instanceDir ? `${instanceDir}/shaderpacks` : undefined
  );
  const shadersCount = instance != null ? String(shaderEntries.length) : "—";

  return (
    <motion.div
      variants={fadeSlideUp}
      initial="initial"
      animate="animate"
      transition={{ delay: 0.15 }}
      className="h-full"
    >
      <Card className="h-full relative overflow-hidden shadow-sm hover:shadow-xl transition-all duration-300 cursor-pointer flex flex-col border">
      <CardHeader className="relative">
        <div className="w-14 h-14 mb-4 flex items-center justify-center rounded-xl bg-muted">
          <Cpu className="size-8 text-muted-foreground" />
        </div>
        <CardTitle className="text-xl font-bold">实例信息</CardTitle>
        <CardDescription>查看实例详情</CardDescription>
      </CardHeader>

      <CardContent className="relative flex-1 space-y-3">
        <InfoItem
          label="存档数量"
          value={worldsCount}
          tooltip="saves/ 目录中的世界存档数量"
          icon={<FolderOpen className="size-4" />}
        />
        <InfoItem
          label="资源包数量"
          value={resourcePacksCount}
          tooltip="resourcepacks/ 目录中的材质包数量"
          icon={<ImageIcon className="size-4" />}
        />
        <InfoItem
          label="游戏版本"
          value={mcVersion}
          tooltip="Minecraft 版本号"
          icon={<MessageSquare className="size-4" />}
        />
        <InfoItem
          label="加载器"
          value={loader}
          tooltip="模组加载器"
          icon={<BadgeCheck className="size-4" />}
        />
        <InfoItem
          label="模组数量"
          value={modsCount}
          tooltip="mods/ 目录中的文件数量"
          icon={<Smile className="size-4" />}
        />
        <InfoItem
          label="光影包数量"
          value={shadersCount}
          tooltip="shaderpacks/ 目录中的光影包数量"
          icon={<Sparkles className="size-4" />}
        />
      </CardContent>
    </Card>
    </motion.div>
  );
}
