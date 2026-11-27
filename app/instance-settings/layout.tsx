"use client";

import { NavLink } from "@/components/nav-link";
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
  const isActive = (href: string) => {
    const target = href.replace(/\/+$/, "");
    return current === target || current.startsWith(`${target}/`);
  };

  return (
    <div className="h-full flex flex-col">
      <div className="shrink-0 flex items-center gap-2 px-3 py-2 border-b bg-background/80 backdrop-blur-sm">
        <div className="flex gap-0.5 overflow-x-auto">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.href}
              href={item.href}
              className={cn(
                "px-3 py-1 rounded-md text-xs font-medium transition-colors whitespace-nowrap",
                isActive(item.href)
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:text-foreground hover:bg-accent/50"
              )}
            >
              {item.label}
            </NavLink>
          ))}
        </div>
      </div>
      <div className="flex-1 overflow-hidden">{children}</div>
    </div>
  );
}
