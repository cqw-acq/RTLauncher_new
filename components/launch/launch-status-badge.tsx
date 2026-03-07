"use client";

import { AnimatePresence, motion } from "framer-motion";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { fadeIn } from "@/lib/motion";
import {
  Circle,
  Loader2,
  Play,
  AlertCircle,
  CheckCircle2,
  Square,
} from "lucide-react";
import type { LaunchStatus } from "@/types";

const statusConfig: Record<
  LaunchStatus,
  { label: string; icon: React.ReactNode; color: string }
> = {
  idle: {
    label: "就绪",
    icon: <Circle className="size-3" />,
    color: "text-muted-foreground",
  },
  preparing: {
    label: "准备中",
    icon: <Loader2 className="size-3 animate-spin" />,
    color: "text-blue-500",
  },
  launching: {
    label: "启动中",
    icon: <Loader2 className="size-3 animate-spin" />,
    color: "text-amber-500",
  },
  running: {
    label: "运行中",
    icon: <Play className="size-3" />,
    color: "text-green-500",
  },
  stopped: {
    label: "已停止",
    icon: <Square className="size-3" />,
    color: "text-muted-foreground",
  },
  error: {
    label: "错误",
    icon: <AlertCircle className="size-3" />,
    color: "text-destructive",
  },
};

interface LaunchStatusBadgeProps {
  status: LaunchStatus;
  className?: string;
}

export function LaunchStatusBadge({ status, className }: LaunchStatusBadgeProps) {
  const cfg = statusConfig[status];
  return (
    <Badge
      variant="outline"
      className={cn("gap-1.5 overflow-hidden", cfg.color, className)}
    >
      <AnimatePresence mode="wait" initial={false}>
        <motion.span
          key={status}
          variants={fadeIn}
          initial="initial"
          animate="animate"
          exit="exit"
          className="flex items-center gap-1.5"
        >
          {cfg.icon}
          {cfg.label}
        </motion.span>
      </AnimatePresence>
    </Badge>
  );
}
