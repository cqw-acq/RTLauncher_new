"use client";

import { useSyncExternalStore } from "react";
import { Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";

import { Button } from "@/components/ui/button";
import { useSettings } from "@/components/settings/settings-provider";

export function ModeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  const { update } = useSettings();
  // 服务端/首帧渲染 false，水合完成后 true，避免 title 等属性因主题解析不同步导致水合不匹配
  const mounted = useSyncExternalStore(
    () => () => {},
    () => true,
    () => false
  );

  const isDark = mounted && resolvedTheme === "dark";
  const label = mounted ? (isDark ? "切换到浅色模式" : "切换到深色模式") : "切换主题";

  const handleToggle = () => {
    if (!mounted || !resolvedTheme) return;
    const next: "light" | "dark" = isDark ? "light" : "dark";
    setTheme(next);
    update("appearance", { themeMode: next });
  };

  return (
    <Button
      variant="ghost"
      size="icon"
      className="h-8 w-8 rounded-full"
      onClick={handleToggle}
      title={label}
      aria-label={label}
    >
      <Sun className="h-4 w-4 scale-100 rotate-0 transition-all dark:scale-0 dark:-rotate-90" />
      <Moon className="absolute h-4 w-4 scale-0 rotate-90 transition-all dark:scale-100 dark:rotate-0" />
    </Button>
  );
}
