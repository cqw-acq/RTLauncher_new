"use client";

import { useState, useEffect } from "react";
import { AccountSwitcher } from "@/components/accounts/account-switcher";
import { useAccountContext } from "@/components/accounts/account-provider";
import { AnnouncementCard } from "@/components/home/announcement-card";
import { ProfileCard } from "@/components/home/profile-card";
import { SkinPreview } from "@/components/home/skin-preview";
import type { Account } from "@/types";

/**
 * 主页组件
 */
export default function Home() {
  const [isProfileSelectorOpen, setIsProfileSelectorOpen] = useState(false);
  const { selectedProfile, selectProfile } = useAccountContext();

  const handleProfileSelect = (profile: Account) => {
    selectProfile(profile);
  };

  useEffect(() => {
    document.body.classList.add("no-scrollbar");
    document.body.style.overflow = "hidden";

    return () => {
      document.body.classList.remove("no-scrollbar");
      document.body.style.overflow = "";
    };
  }, []);

  return (
    <div className="relative h-full overflow-hidden">
      {/* 主体：三栏布局 —— 左：公告（方形） / 中：3D 皮肤预览 / 右：账户+启动 */}
      <div className="h-full w-full overflow-y-auto p-4 md:p-5">
        <div className="grid h-full gap-4 md:gap-5 lg:grid-cols-[minmax(220px,280px)_1fr_minmax(260px,320px)]">
          {/* 左栏：公告 */}
          <div className="flex flex-col gap-4 min-h-0">
            <AnnouncementCard />
          </div>

          {/* 中栏：3D 皮肤预览 */}
          <div className="flex items-center justify-center min-h-0">
            <SkinPreview profile={selectedProfile} />
          </div>

          {/* 右栏：个人资料 + 启动按钮 */}
          <div className="flex flex-col gap-4 min-h-0">
            <ProfileCard
              selectedProfile={selectedProfile}
              onOpenProfileSelector={() => setIsProfileSelectorOpen(true)}
            />
          </div>
        </div>
      </div>

      {/* 账户切换弹窗 */}
      <AccountSwitcher
        open={isProfileSelectorOpen}
        onClose={() => setIsProfileSelectorOpen(false)}
        onSelect={handleProfileSelect}
      />
    </div>
  );
}