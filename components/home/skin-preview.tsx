"use client";

import { motion } from "framer-motion";
import { SkinViewer3D } from "@/components/accounts/skin-viewer-3d";
import { Card, CardContent } from "@/components/ui/card";
import { UserPlus } from "lucide-react";
import type { Account } from "@/types";

type SkinPreviewProps = {
  profile: Account | null;
  onOpenProfileSelector?: () => void;
};

/**
 * 主页中间大尺寸 3D 皮肤预览
 * - 居中展示，占据主要视觉空间
 * - 有皮肤：3D 渲染
 * - 无皮肤/未登录：显示"点击创建账户"提示
 */
export function SkinPreview({ profile }: SkinPreviewProps) {
  const hasSkin = !!profile?.skinUrl;
  const displayName = profile?.name ?? "尚未登录";

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: 0.08, duration: 0.35 }}
      className="h-full w-full flex items-center justify-center"
    >
      <div className="h-full w-full flex flex-col items-center justify-center gap-3 p-4">
        {/* 3D 皮肤展示 —— 无边框 */}
        <div className="flex flex-1 items-center justify-center w-full min-h-0">
          {hasSkin ? (
            <div className="relative rounded-lg bg-muted/40 overflow-hidden">
              <SkinViewer3D
                skinSrc={profile!.skinUrl!}
                width={420}
                height={520}
              />
            </div>
          ) : profile ? (
            // 已登录但无皮肤：显示首字母占位
            <div className="relative flex flex-col items-center justify-center gap-3">
              <div className="size-24 rounded-2xl bg-muted flex items-center justify-center text-3xl font-semibold text-muted-foreground shadow-sm">
                {displayName.charAt(0).toUpperCase()}
              </div>
            </div>
          ) : (
            // 完全未登录：邀请创建账户
            <div className="relative flex flex-col items-center justify-center gap-3 text-muted-foreground">
              <div className="size-24 rounded-2xl bg-muted flex items-center justify-center shadow-sm">
                <UserPlus className="size-10" />
              </div>
              <div className="text-center text-sm">
                点击右侧「管理账户」添加你的角色
              </div>
            </div>
          )}
        </div>
      </div>
    </motion.div>
  );
}