"use client";

import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import { useI18n, type Translation } from "@/components/i18n/use-i18n";

const NAV_ITEMS = [
  { href: "/game-settings/mods/", label: "panel.mods" as Translation },
  { href: "/game-settings/resources/", label: "home.cardGrid.resourcePacks" as Translation },
  { href: "/game-settings/worlds/", label: "gameSettings.worlds" as Translation },
  { href: "/game-settings/shaders/", label: "home.cardGrid.shaders" as Translation },
  { href: "/game-settings/datapacks/", label: "gameSettings.datapacks" as Translation },
  { href: "/game-settings/schematics/", label: "home.cardGrid.schematics" as Translation },
  { href: "/game-settings/screenshots/", label: "home.cardGrid.screenshots" as Translation },
];

export default function GameSettingsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const pathname = usePathname();
  const { t } = useI18n();
  const current = pathname.replace(/\/+$/, "");

  return (
    <div className="h-full flex flex-col">
      {/* 顶部导航栏 */}
      <div className="shrink-0 flex items-center gap-2 px-3 py-2 border-b bg-background/80 backdrop-blur-sm">
        {/* 子页面 Tab 导航：使用原生 <a> 驱动整页导航，避免打包版下 Next 客户端路由不提交的问题 */}
        <div className="flex gap-0.5 overflow-x-auto">
          {NAV_ITEMS.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className={cn(
                "px-3 py-1 rounded-md text-xs font-medium transition-colors whitespace-nowrap",
                current === item.href.replace(/\/+$/, "")
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
              )}
            >
              {t(item.label)}
            </a>
          ))}
        </div>
      </div>
      {/* 页面内容 */}
      <div className="flex-1 overflow-hidden">{children}</div>
    </div>
  );
}