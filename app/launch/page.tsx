"use client";

import { Rocket } from "lucide-react";
import { motion } from "framer-motion";
import { LaunchConfigCard } from "@/components/launch/launch-config-card";
import { LaunchPanel } from "@/components/launch/launch-panel";
import { LaunchConsole } from "@/components/launch/launch-console";
import { fadeSlideUp } from "@/lib/motion";

/**
 * 启动页面
 * 配置启动参数 → 启动游戏 → 查看日志
 */
export default function LaunchPage() {
  return (
    <div className="flex h-full flex-col gap-4 p-4 overflow-y-auto">
      {/* 页面标题 */}
      <div className="flex items-center gap-3 shrink-0">
        <div className="flex size-9 items-center justify-center rounded-xl bg-primary/10">
          <Rocket className="size-5 text-primary" />
        </div>
        <div>
          <h1 className="text-lg font-semibold leading-none">游戏启动</h1>
          <p className="mt-1 text-xs text-muted-foreground">
            选择版本，配置启动参数，开始你的 Minecraft 之旅
          </p>
        </div>
      </div>

      {/* 主内容区 */}
      <div className="flex flex-col lg:flex-row gap-4 flex-1 min-h-0">
        {/* 左侧 - 启动配置 */}
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          transition={{ delay: 0 }}
          className="w-full lg:w-1/2 xl:w-3/5 overflow-y-auto min-h-0"
        >
          <LaunchConfigCard />
        </motion.div>

        {/* 右侧 - 启动面板 + 日志 */}
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          transition={{ delay: 0.1 }}
          className="w-full lg:w-1/2 xl:w-2/5 flex flex-col gap-4 min-h-0"
        >
          <LaunchPanel />
          <div className="flex-1 min-h-0">
            <LaunchConsole />
          </div>
        </motion.div>
      </div>
    </div>
  );
}
