"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Avatar, AvatarFallback } from "@/components/ui/avatar";
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
    <Card className="transition-all duration-700 ease-in-out h-full flex flex-col justify-between group relative overflow-hidden border hover:border-primary/50 hover:shadow-xl">
      {/* 发光效果背景 */}
      <div
        className="absolute inset-0 opacity-0 group-hover:opacity-20 transition-opacity duration-300 pointer-events-none"
        style={{
          background:
            "linear-gradient(to right, rgb(59, 130, 246), rgb(6, 182, 212))",
        }}
      />
      {/* 卡片边框发光效果 */}
      <div
        className="absolute inset-0 rounded-2xl opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none"
        style={{
          boxShadow:
            "inset 0 0 20px rgba(59, 130, 246, 0.3), inset 0 0 40px rgba(6, 182, 212, 0.2)",
        }}
      />
      {/* 卡片主要内容区域 */}
      <CardContent className="flex-grow flex flex-col items-center justify-center relative z-10">
        <button
          type="button"
          className="cursor-pointer transition-all duration-300 flex items-center gap-3 p-3 w-full rounded-xl hover:bg-accent"
          onClick={onOpenProfileSelector}
        >
          <Avatar>
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
      <CardContent className="flex items-center relative z-10">
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
  );
}
