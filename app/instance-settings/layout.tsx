"use client";

import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";

const NAV_ITEMS = [
  { href: "/instance-settings/mods/", label: "Mods" },
  { href: "/instance-settings/worlds/", label: "世界" },
  { href: "/instance-settings/resources/", label: "资源包" },
  { href: "/instance-settings/shaders/", label: "光影包" },
  { href: "/instance-settings/screenshots/", label: "截图" },
  { href: "/instance-settings/schematics/", label: "投影原理图" },
];

export default function InstanceSettingsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const pathname = usePathname();
  const current = pathname.replace(/\/+$/, "");

  return (
    <div className="h-full flex flex-col">
      {/* 顶部导航栏 */}
      <div className="shrink-0 flex items-center gap-2 px-3 py-2 border-b bg-background/80 backdrop-blur-sm">
        {/* 子页面 Tab 导航：使用原生 <a> 驱动整页导航 */}
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
              {item.label}
            </a>
          ))}
        </div>
      </div>
      {/* 页面内容 */}
      <div className="flex-1 overflow-hidden">{children}</div>
    </div>
  );
}