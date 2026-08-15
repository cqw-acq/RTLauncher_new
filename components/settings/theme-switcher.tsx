"use client";

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useMemo, useState } from "react";
import { FolderCode, PackagePlus, RefreshCw, Trash2 } from "lucide-react";

import { useI18n } from "@/components/i18n/use-i18n";
import { useSettings } from "@/components/settings/settings-provider";
import { useThemeRuntime } from "@/components/themes/theme-runtime-provider";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { BUILTIN_THEME_ID } from "@/lib/themes/protocol";

export interface ThemeSwitcherOperations {
  pickArchive(): Promise<string | null>;
  pickDevelopmentDirectory(): Promise<string | null>;
  installArchive(path: string): Promise<void>;
  registerDevelopmentDirectory(path: string): Promise<void>;
  removeTheme(themeId: string): Promise<void>;
  confirm(message: string): Promise<boolean>;
}

const DEFAULT_OPERATIONS: ThemeSwitcherOperations = {
  async pickArchive() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "RTLauncher Theme", extensions: ["rtltheme"] }],
    });
    return typeof selected === "string" ? selected : null;
  },
  async pickDevelopmentDirectory() {
    const selected = await open({ multiple: false, directory: true });
    return typeof selected === "string" ? selected : null;
  },
  installArchive: (archivePath) => invoke("theme_install_archive", { archivePath }),
  registerDevelopmentDirectory: (directory) => invoke(
    "theme_register_dev_directory",
    { directory },
  ),
  removeTheme: (themeId) => invoke("theme_remove", { themeId }),
  async confirm(message) { return window.confirm(message); },
};

export function ThemeSwitcher({
  operations = DEFAULT_OPERATIONS,
}: {
  operations?: ThemeSwitcherOperations;
}) {
  const { t } = useI18n();
  const { update } = useSettings();
  const theme = useThemeRuntime();
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const packages = useMemo(() => {
    const byId = new Map<string, (typeof theme.packages)[number]>();
    theme.packages.forEach((item) => {
      if (!byId.has(item.manifest.id)) byId.set(item.manifest.id, item);
    });
    return [...byId.values()];
  }, [theme.packages]);
  const activePackage = packages.find(
    (item) => item.manifest.id === theme.snapshot.activeThemeId,
  );

  const run = async (operation: () => Promise<void>) => {
    setBusy(true);
    setMessage(null);
    try {
      await operation();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  };

  const selectTheme = async (themeId: string) => run(async () => {
    if (themeId !== BUILTIN_THEME_ID) {
      const trustKey = `rtlauncher:theme-trust:${themeId}`;
      if (localStorage.getItem(trustKey) !== "accepted") {
        const accepted = await operations.confirm(t("settings.themeManager.trustWarning"));
        if (!accepted) return;
        localStorage.setItem(trustKey, "accepted");
      }
    }
    const switched = await theme.activateTheme(themeId);
    if (!switched) {
      setMessage(t("settings.themeManager.switchFailed"));
      return;
    }
    update("appearance", { themeId });
  });

  const installArchive = () => run(async () => {
    const path = await operations.pickArchive();
    if (!path) return;
    await operations.installArchive(path);
    await theme.refreshThemes();
  });

  const registerDevelopmentDirectory = () => run(async () => {
    const path = await operations.pickDevelopmentDirectory();
    if (!path) return;
    await operations.registerDevelopmentDirectory(path);
    await theme.refreshThemes();
  });

  const removeActiveTheme = () => run(async () => {
    const themeId = theme.snapshot.activeThemeId;
    if (themeId === BUILTIN_THEME_ID) return;
    if (!await operations.confirm(t("settings.themeManager.removeWarning"))) return;
    if (!await theme.activateTheme(BUILTIN_THEME_ID)) {
      setMessage(t("settings.themeManager.switchFailed"));
      return;
    }
    update("appearance", { themeId: BUILTIN_THEME_ID });
    await operations.removeTheme(themeId);
    localStorage.removeItem(`rtlauncher:theme-trust:${themeId}`);
    await theme.refreshThemes();
  });

  return (
    <div className="space-y-3">
      <div className="space-y-2">
        <Label htmlFor="frontend-theme">{t("settings.themeManager.selectTheme")}</Label>
        <select
          id="frontend-theme"
          aria-label={t("settings.themeManager.selectTheme")}
          value={theme.snapshot.activeThemeId}
          disabled={busy || !theme.ready}
          onChange={(event) => void selectTheme(event.target.value)}
          className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm"
        >
          <option value={BUILTIN_THEME_ID}>{t("settings.themeManager.builtIn")}</option>
          {packages.map((item) => (
            <option key={item.manifest.id} value={item.manifest.id}>
              {item.manifest.name} · {item.manifest.version}
              {item.development ? ` · ${t("settings.themeManager.development")}` : ""}
            </option>
          ))}
        </select>
      </div>

      <p className="text-xs text-muted-foreground">
        {t("settings.themeManager.description")}
      </p>

      <div className="flex flex-wrap gap-2">
        <Button type="button" size="sm" variant="outline" disabled={busy} onClick={installArchive}>
          <PackagePlus className="size-4" />
          {t("settings.themeManager.install")}
        </Button>
        <Button type="button" size="sm" variant="outline" disabled={busy} onClick={registerDevelopmentDirectory}>
          <FolderCode className="size-4" />
          {t("settings.themeManager.addDevelopment")}
        </Button>
        {activePackage?.development && (
          <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => void run(
            async () => { await theme.reloadTheme(activePackage.manifest.id); },
          )}>
            <RefreshCw className="size-4" />
            {t("settings.themeManager.reload")}
          </Button>
        )}
        {theme.snapshot.activeThemeId !== BUILTIN_THEME_ID && (
          <Button type="button" size="sm" variant="destructive" disabled={busy} onClick={removeActiveTheme}>
            <Trash2 className="size-4" />
            {t("settings.themeManager.remove")}
          </Button>
        )}
      </div>

      {message && <p role="alert" className="text-xs text-destructive">{message}</p>}
    </div>
  );
}
