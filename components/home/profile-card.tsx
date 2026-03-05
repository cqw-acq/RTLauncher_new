"use client";

import { motion } from "framer-motion";
import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { fadeSlideUp } from "@/lib/motion";
import type { Account } from "@/types";

type ProfileCardProps = {
  selectedProfile: Account | null;
  onOpenProfileSelector: () => void;
};

export function ProfileCard({
  selectedProfile,
  onOpenProfileSelector,
}: ProfileCardProps) {
  return (
    <motion.div
      variants={fadeSlideUp}
      initial="initial"
      animate="animate"
      transition={{ delay: 0.1 }}
      className="h-full"
    >
      <Card className="h-full flex flex-col justify-between border shadow-sm hover:shadow-xl transition-shadow duration-300">
        {/* 卡片主要内容区域 */}
        <CardContent className="flex-grow flex flex-col items-center justify-center">
          <button
            type="button"
            className="cursor-pointer transition-all duration-300 flex items-center gap-3 p-3 w-full rounded-xl hover:bg-accent"
            onClick={onOpenProfileSelector}
          >
            <Avatar>
              {selectedProfile?.skinUrl && (
                <AvatarImage src={selectedProfile.skinUrl} alt={selectedProfile.name} />
              )}
              <AvatarFallback>
                {(selectedProfile?.name ?? "RTL User").charAt(0).toUpperCase()}
              </AvatarFallback>
            </Avatar>
            <div className="flex flex-col text-left">
              <span className="font-bold text-base">
                {selectedProfile?.name ?? "RTL User"}
              </span>
              <span className="text-muted-foreground text-xs">
                {selectedProfile?.status ?? ""}
              </span>
            </div>
          </button>
        </CardContent>

        {/* 卡片底部按钮区域 */}
        <CardContent className="flex items-center">
          <div className="w-full">
            <div className="text-center text-sm text-muted-foreground mb-2">
              1.21.7 Fabric
            </div>
            <Button variant="default" size="lg" className="w-full mb-2">
              启动游戏
            </Button>
            <div className="flex gap-2 mt-2">
              <Button variant="secondary" className="flex-1">
                版本管理
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </motion.div>
  );
}
