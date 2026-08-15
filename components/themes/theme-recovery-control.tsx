"use client";

import { RotateCcw } from "lucide-react";

import { useI18n } from "@/components/i18n/use-i18n";
import { Button } from "@/components/ui/button";
import { BUILTIN_THEME_ID } from "@/lib/themes/protocol";
import { useThemeRuntime } from "./theme-runtime-provider";

export function ThemeRecoveryControl() {
  const { t } = useI18n();
  const { snapshot, activateTheme } = useThemeRuntime();
  if (snapshot.activeThemeId === BUILTIN_THEME_ID) return null;

  const label = t("settings.themeManager.recoverBuiltIn");
  return (
    <Button
      type="button"
      size="icon"
      variant="destructive"
      className="fixed bottom-4 left-4 z-[100] shadow-lg"
      aria-label={label}
      title={label}
      onClick={() => void activateTheme(BUILTIN_THEME_ID)}
    >
      <RotateCcw className="size-4" />
    </Button>
  );
}
