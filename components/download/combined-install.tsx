"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { motion } from "framer-motion";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { LOADER_OPTIONS } from "@/constants/data";
import { canSelectLoader, MOD_LOADER_GROUP } from "@/lib/utils";
import { staggerContainer, staggerItem } from "@/lib/motion";
import { LoaderIcon } from "@/components/launch/loader-icon";
import { useDownloadManager } from "@/components/download/download-provider";
import { useI18n } from "@/components/i18n/use-i18n";
import { Download, CheckCircle2, Loader2, TriangleAlert } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { LoaderType, LoaderVersion, MinecraftVersion } from "@/types";

const loaderColors: Record<LoaderType, { bg: string; text: string; border: string; card: string }> = {
  vanilla: { bg: "bg-stone-100 dark:bg-stone-800/40", text: "text-stone-600 dark:text-stone-400", border: "hover:border-stone-400 dark:hover:border-stone-500", card: "" },
  forge: { bg: "bg-orange-100 dark:bg-orange-900/30", text: "text-orange-600 dark:text-orange-400", border: "hover:border-orange-400 dark:hover:border-orange-500", card: "" },
  fabric: { bg: "bg-amber-100 dark:bg-amber-900/30", text: "text-amber-600 dark:text-amber-400", border: "hover:border-amber-400 dark:hover:border-amber-500", card: "" },
  quilt: { bg: "bg-purple-100 dark:bg-purple-900/30", text: "text-purple-600 dark:text-purple-400", border: "hover:border-purple-400 dark:hover:border-purple-500", card: "" },
  neoforge: { bg: "bg-red-100 dark:bg-red-900/30", text: "text-red-600 dark:text-red-400", border: "hover:border-red-400 dark:hover:border-red-500", card: "" },
  liteloader: { bg: "bg-cyan-100 dark:bg-cyan-900/30", text: "text-cyan-600 dark:text-cyan-400", border: "hover:border-cyan-400 dark:hover:border-cyan-500", card: "" },
  optifine: { bg: "bg-yellow-100 dark:bg-yellow-900/30", text: "text-yellow-600 dark:text-yellow-400", border: "hover:border-yellow-400 dark:hover:border-yellow-500", card: "" },
};

interface CombinedInstallProps {
  version: MinecraftVersion;
}

type LoaderVersionEntry = LoaderVersion & { filename?: string; official_url?: string };

interface SelectionItem {
  loader: Exclude<LoaderType, "vanilla">;
  version: string;
}

export function CombinedInstall({ version }: CombinedInstallProps) {
  const { t } = useI18n();
  const { startCombinedInstall } = useDownloadManager();

  const [selected, setSelected] = useState<LoaderType[]>([]);
  const [versions, setVersions] = useState<Partial<Record<LoaderType, string>>>({});
  const [versionLists, setVersionLists] = useState<Partial<Record<LoaderType, LoaderVersionEntry[]>>>({});
  const [loadingLists, setLoadingLists] = useState<Partial<Record<LoaderType, boolean>>>({});
  const [fabricApiEnabled, setFabricApiEnabled] = useState(false);
  const [fabricApiVersion, setFabricApiVersion] = useState("");
  const [fabricApiVersions, setFabricApiVersions] = useState<LoaderVersionEntry[]>([]);
  const [loadingFabricApi, setLoadingFabricApi] = useState(false);

  const [showNameDialog, setShowNameDialog] = useState(false);
  const [instanceNameInput, setInstanceNameInput] = useState("");
  const [installing, setInstalling] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const hasModLoader = selected.some((l) => MOD_LOADER_GROUP.includes(l));
  const hasFabric = selected.includes("fabric");

  const fetchVersions = useCallback(
    async (loader: LoaderType) => {
      if (versionLists[loader] || loadingLists[loader]) return;
      setLoadingLists((prev) => ({ ...prev, [loader]: true }));
      try {
        let result: LoaderVersionEntry[] = [];
        switch (loader) {
          case "forge":
            result = (await invoke<{ id: string; version: string }[]>("get_forge_versions", { mcVersion: version.id })).map((v) => ({ id: v.id, version: v.version, releaseDate: "" }));
            break;
          case "neoforge":
            result = (await invoke<{ id: string; version: string }[]>("get_neoforge_versions", { mcVersion: version.id })).map((v) => ({ id: v.id, version: v.version, releaseDate: "" }));
            break;
          case "fabric":
            result = (await invoke<{ id: string; version: string }[]>("get_fabric_loader_versions", { mcVersion: version.id, useMirror: true })).map((v) => ({ id: v.id, version: v.version, releaseDate: "" }));
            break;
          case "quilt":
            result = (await invoke<{ id: string; version: string }[]>("get_quilt_loader_versions", { mcVersion: version.id })).map((v) => ({ id: v.id, version: v.version, releaseDate: "" }));
            break;
          case "liteloader":
            result = (await invoke<{ id: string; version: string }[]>("get_liteloader_versions", { mcVersion: version.id })).map((v) => ({ id: v.id, version: v.version, releaseDate: "" }));
            break;
          case "optifine":
            result = (await invoke<{ id: string; filename: string; self_version: string; download_url: string; official_url: string; is_pre: boolean }[]>("get_optifine_versions", { mcVersion: version.id })).map((v) => ({
              id: v.download_url,
              version: v.self_version,
              filename: v.filename,
              official_url: v.official_url,
              releaseDate: "",
              isRecommended: !v.is_pre,
            }));
            break;
          default:
            return;
        }
        setVersionLists((prev) => ({ ...prev, [loader]: result }));
      } catch (err) {
        console.error(`获取${loader}版本列表失败:`, err);
      } finally {
        setLoadingLists((prev) => ({ ...prev, [loader]: false }));
      }
    },
    [version.id, versionLists, loadingLists]
  );

  const fetchFabricApiVersions = useCallback(async () => {
    if (fabricApiVersions.length > 0 || loadingFabricApi) return;
    setLoadingFabricApi(true);
    try {
      const result = await invoke<{ id: string; version: string }[]>("get_fabric_api_versions", { mcVersion: version.id });
      setFabricApiVersions(result.map((v) => ({ id: v.id, version: v.version, releaseDate: "" })));
    } catch (err) {
      console.error("获取Fabric API版本列表失败:", err);
    } finally {
      setLoadingFabricApi(false);
    }
  }, [version.id, fabricApiVersions.length, loadingFabricApi]);

  useEffect(() => {
    if (hasFabric) void fetchFabricApiVersions();
  }, [hasFabric, fetchFabricApiVersions]);

  const toggleLoader = (loader: LoaderType) => {
    if (loader === "vanilla") return;
    if (selected.includes(loader)) {
      setSelected((prev) => prev.filter((l) => l !== loader));
      setVersions((prev) => {
        const next = { ...prev };
        delete next[loader];
        return next;
      });
      return;
    }
    const check = canSelectLoader(version.id, loader, selected);
    if (!check.allowed) return;
    setSelected((prev) => [...prev, loader]);
    void fetchVersions(loader);
  };

  const selectableStates = (loader: LoaderType) => {
    if (loader === "vanilla") {
      return { allowed: true, reason: undefined as string | undefined };
    }
    return canSelectLoader(version.id, loader, selected);
  };

  const defaultInstanceName = (() => {
    if (selected.length === 0) return version.id;
    const parts = [version.id];
    for (const l of selected) {
      if (l === "vanilla") continue;
      const v = versions[l];
      parts.push(v ? `${l}-${v}` : l);
    }
    if (fabricApiEnabled) parts.push(fabricApiVersion ? `API-${fabricApiVersion}` : "API");
    return parts.join("-").replace(/[\\/:*?"<>|]/g, "_");
  })();

  const allVersionsChosen = selected
    .filter((l) => l !== "vanilla")
    .every((l) => Boolean(versions[l]));
  const fabricApiReady = !fabricApiEnabled || Boolean(fabricApiVersion);
  const canInstall = allVersionsChosen && fabricApiReady;

  const handleInstall = () => {
    if (!canInstall) return;
    setInstanceNameInput(defaultInstanceName);
    setShowNameDialog(true);
    setTimeout(() => {
      inputRef.current?.focus();
      inputRef.current?.select();
    }, 50);
  };

  const confirmInstall = async () => {
    if (!canInstall) return;
    const name = instanceNameInput.trim();
    const instanceName = name.length > 0 ? name : defaultInstanceName;

    setShowNameDialog(false);
    setInstalling(true);
    try {
      const selections: SelectionItem[] = selected
        .filter((l) => l !== "vanilla")
        .map((l) => ({ loader: l as SelectionItem["loader"], version: versions[l]! }));
      if (fabricApiEnabled) {
        selections.push({ loader: "fabric_api" as unknown as SelectionItem["loader"], version: fabricApiVersion });
      }
      await startCombinedInstall(`Minecraft ${version.id} 组合安装`, version.id, selections, instanceName);
    } catch (err) {
      console.error("组合安装启动失败:", err);
    } finally {
      setInstalling(false);
    }
  };

  const selectedOptions = LOADER_OPTIONS.filter((l) => l.id !== "vanilla" && selected.includes(l.id));

  return (
    <>
      <div className="flex h-full flex-col gap-4">
        {/* 原版：必选 */}
        <div className="shrink-0">
          <div className="flex items-center gap-3 rounded-xl border border-border bg-card px-4 py-3">
            <Switch checked disabled className="border-emerald-500" />
            <div className={cn("flex size-9 items-center justify-center rounded-lg p-1.5", loaderColors.vanilla.bg)}>
              <LoaderIcon kind="vanilla" className="size-full" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="text-sm font-semibold">{t("download.vanillaMandatory")}</span>
                <Badge variant="default" className="text-[10px] px-1.5 py-0">
                  {t("download.mandatory")}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground mt-0.5">
                {version.id}
              </p>
            </div>
          </div>
        </div>

        {/* 可选加载器 */}
        <div className="shrink-0">
          <h3 className="text-sm font-medium text-muted-foreground">
            {t("download.selectLoader")}
          </h3>
          <p className="text-xs text-muted-foreground/70 mt-0.5">
            {t("download.selectLoaderHint")}
          </p>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto p-1">
          <motion.div
            className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3"
            variants={staggerContainer}
            initial="initial"
            animate="animate"
          >
            {LOADER_OPTIONS.filter((l) => l.id !== "vanilla").map((loader) => {
              const state = selectableStates(loader.id);
              const isSelected = selected.includes(loader.id);
              const colors = loaderColors[loader.id];
              const disabled = !state.allowed;
              return (
                <motion.div key={loader.id} variants={staggerItem}>
                  <Card
                    className={cn(
                      "group cursor-pointer transition-all duration-200 shadow-sm h-full",
                      colors.border,
                      disabled && "opacity-45 cursor-not-allowed pointer-events-none",
                      isSelected && "ring-2 ring-primary"
                    )}
                    onClick={() => toggleLoader(loader.id)}
                  >
                    <CardHeader className="p-4 pb-2">
                      <div className="flex items-center justify-between">
                        <div
                          className={cn(
                            "flex size-10 items-center justify-center rounded-xl p-1.5",
                            colors.bg
                          )}
                        >
                          <LoaderIcon kind={loader.id} className="size-full" />
                        </div>
                        <Switch
                          checked={isSelected}
                          disabled={disabled}
                          onClick={(e) => e.stopPropagation()}
                          onCheckedChange={() => toggleLoader(loader.id)}
                          aria-label={loader.name}
                        />
                      </div>
                      <CardTitle className="text-sm mt-2">{loader.name}</CardTitle>
                    </CardHeader>
                    <CardContent className="p-4 pt-0">
                      <CardDescription className="text-xs line-clamp-2">
                        {state.reason ?? loader.description}
                      </CardDescription>
                    </CardContent>
                  </Card>
                </motion.div>
              );
            })}
          </motion.div>

          {/* Fabric API（仅选 Fabric 时出现） */}
          {hasFabric && (
            <motion.div
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              className="mt-4"
            >
              <div className="rounded-xl border border-border bg-card px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3 min-w-0">
                    <Switch
                      checked={fabricApiEnabled}
                      onCheckedChange={setFabricApiEnabled}
                      aria-label="Fabric API"
                    />
                    <div className="min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-semibold">Fabric API</span>
                        <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
                          {t("download.mod")}
                        </Badge>
                      </div>
                      <p className="text-xs text-muted-foreground mt-0.5">
                        {t("download.fabricApiHint")}
                      </p>
                    </div>
                  </div>
                  {fabricApiEnabled && (
                    <div className="w-52 shrink-0">
                      {loadingFabricApi ? (
                        <div className="flex items-center justify-center py-2 text-muted-foreground">
                          <Loader2 className="size-4 animate-spin" />
                        </div>
                      ) : (
                        <Select value={fabricApiVersion} onValueChange={setFabricApiVersion}>
                          <SelectTrigger className="h-9 text-xs">
                            <SelectValue placeholder={t("download.selectVersion")} />
                          </SelectTrigger>
                          <SelectContent>
                            {fabricApiVersions.map((v) => (
                              <SelectItem key={v.id} value={v.id} className="text-xs">
                                {v.version}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      )}
                    </div>
                  )}
                </div>
              </div>
            </motion.div>
          )}

          {/* 已选加载器版本选择 */}
          {selectedOptions.length > 0 && (
            <div className="mt-4 space-y-3">
              {selectedOptions.map((loader) => {
                const colors = loaderColors[loader.id];
                const list = versionLists[loader.id];
                const loading = loadingLists[loader.id];
                return (
                  <motion.div
                    key={loader.id}
                    initial={{ opacity: 0, y: 8 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="rounded-xl border border-border bg-card px-4 py-3"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="flex items-center gap-3 min-w-0">
                        <div className={cn("flex size-9 items-center justify-center rounded-lg p-1.5", colors.bg)}>
                          <LoaderIcon kind={loader.id} className="size-full" />
                        </div>
                        <span className="text-sm font-semibold">{loader.name}</span>
                      </div>
                      <div className="w-60 shrink-0">
                        {loading ? (
                          <div className="flex items-center justify-center py-2 text-muted-foreground">
                            <Loader2 className="size-4 animate-spin" />
                          </div>
                        ) : (
                          <Select
                            value={versions[loader.id] ?? ""}
                            onValueChange={(v) => setVersions((prev) => ({ ...prev, [loader.id]: v }))}
                          >
                            <SelectTrigger className="h-9 text-xs">
                              <SelectValue placeholder={t("download.selectVersion")} />
                            </SelectTrigger>
                            <SelectContent>
                              {(list ?? []).map((v) => (
                                <SelectItem key={v.id} value={v.id} className="text-xs">
                                  {v.version}
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                        )}
                      </div>
                    </div>
                  </motion.div>
                );
              })}
            </div>
          )}
        </div>

        {/* 底部安装按钮 */}
        <div className="shrink-0 border-t border-border pt-4">
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-2 text-xs text-muted-foreground min-w-0">
              {selected.length === 0 ? (
                <>
                  <CheckCircle2 className="size-4 text-emerald-500 shrink-0" />
                  {t("download.vanillaOnly")}
                </>
              ) : (
                <>
                  <CheckCircle2 className="size-4 text-emerald-500 shrink-0" />
                  <span className="truncate">
                    {selected.map((l) => LOADER_OPTIONS.find((o) => o.id === l)?.name ?? l).join(" + ")}
                    {fabricApiEnabled ? " + Fabric API" : ""}
                  </span>
                </>
              )}
            </div>
            <Button
              size="sm"
              disabled={!canInstall || installing}
              onClick={handleInstall}
              className="shrink-0"
            >
              {installing ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Download className="size-4" />
              )}
              {t("download.installCombined")}
            </Button>
          </div>
          {!allVersionsChosen && selected.length > 0 && (
            <div className="flex items-center gap-1.5 mt-2 text-xs text-amber-600 dark:text-amber-400">
              <TriangleAlert className="size-3.5" />
              {t("download.pickAllVersionsHint")}
            </div>
          )}
        </div>
      </div>

      {/* 实例名称输入对话框 */}
      <Dialog open={showNameDialog} onOpenChange={setShowNameDialog}>
        <DialogContent className="!max-w-lg p-0">
          <DialogHeader>
            <DialogTitle>{t("download.instanceName")}</DialogTitle>
          </DialogHeader>
          <div className="p-5 space-y-4">
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">
                {t("download.instanceNameHint")}
                <code className="mx-1 px-1.5 py-0.5 rounded bg-muted text-xs">
                  {defaultInstanceName}
                </code>
              </p>
              <Input
                ref={inputRef}
                placeholder={defaultInstanceName}
                value={instanceNameInput}
                onChange={(e) => setInstanceNameInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void confirmInstall();
                }}
              />
            </div>
            <div className="flex items-center justify-end gap-2">
              <Button
                variant="outline"
                onClick={() => {
                  setShowNameDialog(false);
                }}
              >
                {t("download.cancel")}
              </Button>
              <Button onClick={() => void confirmInstall()}>
                {t("download.startDownload")}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}