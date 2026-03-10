"use client";

import { useState, useEffect } from "react";
import { AccountSwitcher } from "@/components/accounts/account-switcher";
import { useAccountContext } from "@/components/accounts/account-provider";
import { AnnouncementCard } from "@/components/home/announcement-card";
import { ProfileCard } from "@/components/home/profile-card";
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
      {/* 右侧栏 - 个人资料卡片 */}
      <div className="absolute right-0 top-0 w-full md:w-1/3 lg:w-1/4 h-full p-4 pointer-events-none">
        <div className="pointer-events-auto h-full">
          <ProfileCard
            selectedProfile={selectedProfile}
            onOpenProfileSelector={() => setIsProfileSelectorOpen(true)}
          />
        </div>
      </div>

      {/* 公告栏 */}
      <div className="absolute left-0 top-0 w-full md:w-1/3 lg:w-1/4 h-auto p-4 pointer-events-none">
        <div className="pointer-events-auto">
          <AnnouncementCard />
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