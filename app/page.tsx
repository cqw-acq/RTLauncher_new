"use client";

import { Button } from "@/components/ui/button";
import { useState, useEffect, useRef } from "react";
import { InstanceHeader } from "@/components/home/instance-header";
import { AccountSwitcher } from "@/components/accounts/account-switcher";
import { useAccountContext } from "@/components/accounts/account-provider";
import { useViewToggle } from "@/hooks/use-view-toggle";
import { AnnouncementCard } from "@/components/home/announcement-card";
import { ProfileCard } from "@/components/home/profile-card";
import { InstanceInfoCard } from "@/components/home/instance-info-card";
import { InstanceCardGrid } from "@/components/home/instance-card-grid";
import { ChevronDown, ChevronUp } from "lucide-react";
import type { Account } from "@/types";
import { TRANSITION_DURATION } from "@/constants";
import { useInstances } from "@/hooks/use-instances";
import { useLaunchContext } from "@/components/launch/launch-provider";

/**
 * 主页组件
 * 包含主页视图和实例设置视图的切换
 */
export default function Home() {
  const [isProfileSelectorOpen, setIsProfileSelectorOpen] = useState(false);
  const { selectedProfile, selectProfile } = useAccountContext();
  const { currentView, toggleView } = useViewToggle();
  const isUserAction = useRef(false);

  // 动态实例数据
  const { config } = useLaunchContext();
  const instancesPath = config.minecraftPath ? `${config.minecraftPath}/instance` : undefined;
  const { instances } = useInstances(instancesPath);
  // 默认选中第一个实例
  const selectedInstance = instances.length > 0 ? instances[0] : null;

  const handleToggleView = () => {
    isUserAction.current = true;
    toggleView();
  };

  const handleProfileSelect = (profile: Account) => {
    selectProfile(profile);
  };

  useEffect(() => {
    if (currentView === "home") {
      document.body.classList.add("no-scrollbar");
      document.body.style.overflow = "hidden";
    } else {
      document.body.classList.remove("no-scrollbar");
      document.body.style.overflow = "";
    }

    const disableScrollWheel = (e: WheelEvent | TouchEvent) => {
      if (currentView !== "home") return;
      e.preventDefault();
    };

    const onKeyDown = (e: KeyboardEvent) => {
      if (currentView !== "home") return;
      const isSpace =
        e.code === "Space" || e.key === " " || e.key === "Spacebar";
      if (!isSpace) return;
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName || "";
      const isInteractive =
        ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(tag) ||
        (target?.isContentEditable ?? false);
      if (!isInteractive) {
        e.preventDefault();
      }
    };

    if (currentView === "home") {
      window.addEventListener("wheel", disableScrollWheel as EventListener, {
        passive: false,
      });
      window.addEventListener(
        "touchmove",
        disableScrollWheel as EventListener,
        { passive: false }
      );
      window.addEventListener("keydown", onKeyDown);
    }

    return () => {
      window.removeEventListener(
        "wheel",
        disableScrollWheel as EventListener
      );
      window.removeEventListener(
        "touchmove",
        disableScrollWheel as EventListener
      );
      window.removeEventListener("keydown", onKeyDown);
      document.body.classList.remove("no-scrollbar");
      document.body.style.overflow = "";
    };
  }, [currentView]);

  return (
    <div className="relative h-full">
      {/* 主页内容 */}
      <div
        className={`absolute inset-0 transition-transform ease-out no-scrollbar overflow-hidden ${
          currentView === "home" ? "translate-y-0" : "-translate-y-full"
        }`}
        style={{
          transitionDuration: isUserAction.current
            ? `${TRANSITION_DURATION.PAGE_TRANSITION}ms`
            : "0ms",
        }}
        onTransitionEnd={() => {
          isUserAction.current = false;
        }}
      >
        {/* 右侧栏 - 个人资料卡片 */}
        <div className="absolute right-0 top-0 w-full md:w-1/3 lg:w-1/4 h-full p-4 flex flex-col justify-end">
          <ProfileCard
            selectedProfile={selectedProfile}
            onOpenProfileSelector={() => setIsProfileSelectorOpen(true)}
            instanceName={selectedInstance?.name ?? ""}
            versionDisplay={
              selectedInstance
                ? `${selectedInstance.minecraft_version}${selectedInstance.loader ? ` ${selectedInstance.loader}` : ""}`
                : "未选择游戏版本"
            }
          />
        </div>

        {/* 公告栏 */}
        <div className="absolute left-0 top-0 w-full md:w-1/3 lg:w-1/4 h-auto p-4">
          <AnnouncementCard />
        </div>

        {/* 底部切换按钮 */}
        <div className="absolute bottom-4 left-1/2 -translate-x-1/2 z-10">
          <Button
            onClick={handleToggleView}
            className="gap-2 shadow-lg hover:shadow-xl transition-shadow"
            size="lg"
            aria-label="切换到游戏设置"
          >
            <span>切换到游戏设置</span>
            <ChevronDown className="size-4" />
          </Button>
        </div>
      </div>

      {/* 实例设置区域内容 */}
      <div
        className={`absolute inset-0 transition-transform ease-out flex flex-col ${
          currentView === "instance" ? "translate-y-0" : "translate-y-full"
        }`}
        style={{
          transitionDuration: isUserAction.current
            ? `${TRANSITION_DURATION.PAGE_TRANSITION}ms`
            : "0ms",
        }}
        onTransitionEnd={() => {
          isUserAction.current = false;
        }}
      >
        {/* 顶部实例信息头 */}
        <div className="shrink-0 px-4 pt-2 pb-2 z-10 relative">
          <InstanceHeader
            instanceName={selectedInstance?.name ?? "未选择游戏版本"}
            minecraftVersion={selectedInstance ? `Minecraft ${selectedInstance.minecraft_version}` : ""}
            loader={selectedInstance?.loader ?? ""}
            selectedProfile={selectedProfile}
            onOpenProfileSelector={() => setIsProfileSelectorOpen(true)}
            className="p-2"
          />
          {/* 返回主页按钮 */}
          <div className="absolute top-3 left-1/2 -translate-x-1/2 z-20">
            <Button
              variant="outline"
              onClick={handleToggleView}
              className="gap-2 bg-background/80 backdrop-blur-sm shadow-lg hover:shadow-xl transition-shadow"
              size="sm"
              aria-label="返回主页"
            >
              <ChevronUp className="size-3" />
              <span>返回主页</span>
            </Button>
          </div>
        </div>

        {/* 标签页内容 */}
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