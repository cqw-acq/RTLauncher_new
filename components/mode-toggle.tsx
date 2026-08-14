"use client";

import * as React from "react";
import { Moon, Sun } from "lucide-react";
import { useTheme } from "next-themes";

import { Button } from "@/components/ui/button";
import { useSettings, type ThemeMode } from "@/components/settings/settings-provider";

export function ModeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  const { settings, update } = useSettings();

  const handleToggle = () => {
    const isDark =
      resolvedTheme === "dark" ||
      (resolvedTheme === undefined && settings.appearance.themeMode === "dark");
    const next: ThemeMode = isDark ? "light" : "dark";
    setTheme(next);
    update("appearance", { themeMode: next });
  };

  return (
    <Button
      variant="ghost"
      size="icon"
      className="h-8 w-8 rounded-full"
      onClick={handleToggle}
      aria-label="切换主题"
    >
      <Sun className="h-4 w-4 scale-100 rotate-0 transition-all dark:scale-0 dark:-rotate-90" />
      <Moon className="absolute h-4 w-4 scale-0 rotate-90 transition-all dark:scale-100 dark:rotate-0" />
      <span className="sr-only">切换主题</span>
    </Button>
  );
}
