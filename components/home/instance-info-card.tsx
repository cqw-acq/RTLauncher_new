"use client";

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
  Paintbrush,
  Info,
  MessageSquare,
  BadgeCheck,
  Smile,
  Clock,
} from "lucide-react";
import { Cpu } from "lucide-react";

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

export function InstanceInfoCard() {
  return (
    <Card className="h-full group relative overflow-hidden hover:shadow-xl transition-all duration-300 cursor-pointer flex flex-col border hover:border-primary/50">
      {/* 卡片边框发光效果 */}
      <div
        className="absolute inset-0 rounded-2xl opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none"
        style={{
          boxShadow:
            "inset 0 0 20px rgba(59, 130, 246, 0.3), inset 0 0 40px rgba(6, 182, 212, 0.2)",
        }}
      />
      <CardHeader className="relative">
        <div className="w-14 h-14 mb-4 relative">
          <div className="absolute inset-0 bg-gradient-to-r from-blue-500 to-cyan-500 rounded-xl opacity-20" />
          <div className="absolute inset-0 flex items-center justify-center">
            <Cpu className="size-8 text-blue-600 dark:text-blue-400" />
          </div>
        </div>
        <CardTitle className="text-xl font-bold">实例信息</CardTitle>
        <CardDescription>查看实例详情</CardDescription>
      </CardHeader>

      <CardContent className="relative flex-1 space-y-3">
        <InfoItem
          label="实例名称"
          value="RTL World"
          tooltip="Minecraft 实例的显示名称"
          icon={<Paintbrush className="size-4" />}
        />
        <InfoItem
          label="实例 ID"
          value="RTLE-001"
          tooltip="实例的唯一标识符"
          icon={<Info className="size-4" />}
        />
        <InfoItem
          label="游戏版本"
          value="1.21.8"
          tooltip="Minecraft 版本号"
          icon={<MessageSquare className="size-4" />}
        />
        <InfoItem
          label="加载器"
          value="Fabric"
          tooltip="模组加载器"
          icon={<BadgeCheck className="size-4" />}
        />
        <InfoItem
          label="启动次数"
          value="247"
          tooltip="总启动次数"
          icon={<Smile className="size-4" />}
        />
        <InfoItem
          label="上次启动"
          value="2小时前"
          tooltip="最后一次启动的时间"
          icon={<Clock className="size-4" />}
        />
      </CardContent>
    </Card>
  );
}
