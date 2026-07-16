"use client";

import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { AppearanceSection } from "@/components/settings/section-appearance";
import { AboutSection } from "@/components/settings/section-about";
import { Settings, Sparkles, Package } from "lucide-react";

interface NavItem {
  id: string;
  label: string;
  icon: React.ReactNode;
}

const NAV_ITEMS: NavItem[] = [
  { id: "section-appearance", label: "外观", icon: <Sparkles className="size-4" /> },
  { id: "section-about", label: "版本更新", icon: <Package className="size-4" /> },
];

export default function SettingsPage() {
  const [activeId, setActiveId] = useState<string>(NAV_ITEMS[0].id);

  // 使用 IntersectionObserver 自动高亮当前可见区域
  useEffect(() => {
    const scroller = document.querySelector<HTMLElement>("[data-settings-content]");
    if (!scroller) return;

    const observer = new IntersectionObserver(
      (entries) => {
        let best: IntersectionObserverEntry | null = null;
        for (const e of entries) {
          if (e.isIntersecting) {
            if (!best || e.intersectionRatio > best.intersectionRatio) best = e;
          }
        }
        if (best) setActiveId(best.target.id);
      },
      {
        root: scroller,
        rootMargin: "-30% 0px -55% 0px",
        threshold: [0, 0.1, 0.25, 0.5, 0.75, 1],
      }
    );

    for (const item of NAV_ITEMS) {
      const el = document.getElementById(item.id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  }, []);

  const goTo = (id: string) => {
    const el = document.getElementById(id);
    const scroller = document.querySelector<HTMLElement>("[data-settings-content]");
    if (el && scroller) {
      const top = el.offsetTop - 16;
      scroller.scrollTo({ top, behavior: "smooth" });
      setActiveId(id);
    }
  };

  return (
    <div className="relative h-full overflow-hidden">
      {/* 页面标题 */}
      <div className="border-b border-border bg-background/60 px-5 py-3 backdrop-blur">
        <div className="flex items-center gap-2">
          <div className="flex size-7 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <Settings className="size-3.5" />
          </div>
          <div>
            <h1 className="text-sm font-semibold leading-tight">设置</h1>
            <p className="text-xs text-muted-foreground">全局设置 —— 外观、主题、版本信息</p>
          </div>
        </div>
      </div>

      {/* 主体：两栏布局 */}
      <div className="flex h-[calc(100%-54px)]">
        {/* 左侧锚点导航 */}
        <nav className="hidden w-52 shrink-0 border-r border-border bg-background/30 p-3 md:block">
          <div className="sticky top-0 space-y-1">
            <div className="px-2 pb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
              分类
            </div>
            {NAV_ITEMS.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => goTo(item.id)}
                className={cn(
                  "flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-xs transition-colors",
                  activeId === item.id
                    ? "bg-primary/10 text-primary font-medium"
                    : "text-foreground/80 hover:bg-accent/40"
                )}
              >
                {item.icon}
                <span>{item.label}</span>
              </button>
            ))}
          </div>
        </nav>

        {/* 右侧内容区 */}
        <main
          data-settings-content
          className="h-full flex-1 overflow-y-auto px-4 py-4 md:px-6"
        >
          <div className="mx-auto max-w-2xl space-y-4">
            <AppearanceSection />
            <AboutSection />
            <div className="py-3 text-center text-xs text-muted-foreground">
              — 已经到底了 —
            </div>
          </div>
        </main>
      </div>
    </div>
  );
}