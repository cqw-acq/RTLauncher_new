"use client";

import { InstanceHeader } from "@/components/home/instance-header";
import { InstanceInfoCard } from "@/components/home/instance-info-card";
import { InstanceCardGrid } from "@/components/home/instance-card-grid";
import { AccountSwitcher } from "@/components/accounts/account-switcher";
import { VersionSelectorDialog } from "@/components/launch/version-selector-dialog";
import { useAccountContext } from "@/components/accounts/account-provider";
import { useState } from "react";
import type { Account } from "@/types";
import { useInstances } from "@/hooks/use-instances";
import { useLaunchContext } from "@/components/launch/launch-provider";

/**
 * 游戏设置页面
 */
export default function GameSettings() {
  const [isProfileSelectorOpen, setIsProfileSelectorOpen] = useState(false);
  const [isVersionSelectorOpen, setIsVersionSelectorOpen] = useState(false);
  const { selectedProfile, selectProfile } = useAccountContext();

  // 动态实例数据
  const { config } = useLaunchContext();
  const instancesPath = config.minecraftPath ? `${config.minecraftPath}/instance` : undefined;
  const { instances } = useInstances(instancesPath);
  // 优先匹配当前选中版本，否则选第一个
  const selectedInstance = instances.find((i) => i.name === config.versionName)
    ?? instances[0]
    ?? null;

  const handleProfileSelect = (profile: Account) => {
    selectProfile(profile);
  };

  return (
    <div className="flex flex-col h-full">
      {/* 顶部实例信息头 */}
      <div className="shrink-0 px-4 pt-2 pb-2">
        <InstanceHeader
          instanceName={selectedInstance?.name ?? "未选择游戏版本"}
          minecraftVersion={selectedInstance ? `Minecraft ${selectedInstance.minecraft_version}` : ""}
          loader={selectedInstance?.loader ?? ""}
          selectedProfile={selectedProfile}
          onOpenProfileSelector={() => setIsProfileSelectorOpen(true)}
          onOpenVersionSelector={() => setIsVersionSelectorOpen(true)}
          className="p-2"
        />
      </div>

      {/* 内容区域 */}
      <div className="flex-1 px-4 overflow-y-auto pb-4 min-h-0">
        <div className="flex flex-col md:flex-row gap-4 h-full min-h-0">
          {/* 左侧实例信息 */}
          <div className="w-full md:w-1/3 lg:w-1/4">
            <InstanceInfoCard instance={selectedInstance} />
          </div>
          {/* 右侧卡片网格 */}
          <InstanceCardGrid />
        </div>
      </div>

      {/* 账户切换弹窗 */}
      <AccountSwitcher
        open={isProfileSelectorOpen}
        onClose={() => setIsProfileSelectorOpen(false)}
        onSelect={handleProfileSelect}
      />

      {/* 版本选择弹窗 */}
      <VersionSelectorDialog
        open={isVersionSelectorOpen}
        onOpenChange={setIsVersionSelectorOpen}
      />
    </div>
  );
}
