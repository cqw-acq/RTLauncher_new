"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { ChevronLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const NAV_ITEMS = [
  { href: "/instance-settings/mods", label: "Mods" },
  { href: "/instance-settings/worlds", label: "世界" },
  { href: "/instance-settings/resources", label: "资源包" },
  { href: "/instance-settings/shaders", label: "光影包" },
  { href: "/instance-settings/screenshots", label: "截图" },
  { href: "/instance-settings/schematics", label: "投影原理图" },
];

export default function InstanceSettingsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const pathname = usePathname();

  return (
    <div className="h-full flex flex-col">
      {/* 顶部导航栏 */}
      <div className="shrink-0 flex items-center gap-2 px-3 py-2 border-b bg-background/80 backdrop-blur-sm">
        <Button variant="ghost" size="sm" asChild className="h-7 px-2">
          <Link href="/">
            <ChevronLeft className="size-3.5" />
            <span className="text-xs">返回</span>
          </Link>
        </Button>
        <div className="w-px h-4 bg-border" />
        {/* 子页面 Tab 导航 */}
        <div className="flex gap-0.5 overflow-x-auto">
          {NAV_ITEMS.map((item) => (
            <Link
              key={item.href}
              href={item.href}
              className={cn(
                "px-3 py-1 rounded-md text-xs font-medium transition-colors whitespace-nowrap",
                pathname === item.href
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
              )}
            >
              {item.label}
            </Link>
          ))}
        </div>
      </div>

      {/* 页面内容 */}
      <div className="flex-1 overflow-hidden">{children}</div>
    </div>
  );
}
