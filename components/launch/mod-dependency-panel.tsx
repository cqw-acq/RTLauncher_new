"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  AlertTriangle,
  CheckCircle,
  XCircle,
  RefreshCw,
  Download,
  Puzzle,
  Package,
  Info,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { useI18n } from "@/components/i18n/use-i18n";
import { useInstancePath } from "@/hooks/use-instance-path";
import { cn } from "@/lib/utils";

interface ModDependencyIssue {
  mod_id: string;
  mod_name: string;
  issue_type: "missing" | "version_mismatch" | "incompatible" | "optional";
  required_by: string[];
  version?: string;
  recommended_version?: string;
}

interface ModDependencyAnalysis {
  total_mods: number;
  missing_dependencies: ModDependencyIssue[];
  version_mismatches: ModDependencyIssue[];
  incompatible_mods: ModDependencyIssue[];
  all_resolved: boolean;
}

interface LoaderDependencyError {
  error_type: "missing" | "version_mismatch" | "incompatible";
  mod_id: string;
  required_version?: string;
  required_by?: string;
  message: string;
}

interface LoaderLogReport {
  has_crash: boolean;
  errors: LoaderDependencyError[];
  summary: string;
}

const PLATFORM_DEPS = new Set([
  "minecraft", "forge", "neoforge", "fabricloader", "fabric", "fabric-api",
  "quilt_loader", "java", "liteloader", "minecraftforge", "fabric-api-base",
  "fabric-api-lookup-api-v1", "fabric-biome-api-v1", "fabric-block-api-v1",
  "fabric-blockrenderlayer-v1", "fabric-command-api-v1", "fabric-command-api-v2",
  "fabric-content-registries-v0", "fabric-crash-report-info-v1",
  "fabric-data-attachment-api-v1", "fabric-dimensions-v1", "fabric-entity-events-v1",
  "fabric-events-interaction-v0", "fabric-events-lifecycle-v0",
  "fabric-game-rule-api-v1", "fabric-gametest-api-v1", "fabric-item-api-v1",
  "fabric-item-groups-v0", "fabric-key-binding-api-v1", "fabric-lifecycle-events-v1",
  "fabric-loot-api-v2", "fabric-message-api-v1", "fabric-mining-level-api-v1",
  "fabric-model-loading-api-v1", "fabric-models-v0", "fabric-networking-api-v1",
  "fabric-networking-v0", "fabric-object-builder-api-v1", "fabric-particles-v1",
  "fabric-recipe-api-v1", "fabric-registry-sync-v0", "fabric-renderer-api-v1",
  "fabric-renderer-indigo", "fabric-rendering-fluids-v1", "fabric-rendering-v0",
  "fabric-rendering-v1", "fabric-resource-loader-v0", "fabric-screen-api-v1",
  "fabric-screen-handler-api-v1", "fabric-sound-api-v1", "fabric-textures-v0",
  "fabric-transfer-api-v1", "fabric-transitive-access-wideners-v1",
  "forgeconfigapiport", "kotlinforforge", "architectury",
  "mixinextras", "jni", "com_github_llamalad7_mixinextras",
]);
function is_platform_dep(mod_id: string): boolean {
  return PLATFORM_DEPS.has(mod_id.toLowerCase().replace(/[^a-z0-9_.-]/g, ""));
}

export function ModDependencyPanel() {
  const { t } = useI18n();
  const { config, logs } = useLaunchContext();
  const { instanceDir, loading: instanceLoading } = useInstancePath();

  const [analysis, setAnalysis] = useState<ModDependencyAnalysis | null>(null);
  const [loaderReport, setLoaderReport] = useState<LoaderLogReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [deepAnalyzing, setDeepAnalyzing] = useState(false);
  const [isDeepAnalysis, setIsDeepAnalysis] = useState(false);
  const [downloadingMods, setDownloadingMods] = useState<Record<string, string>>({});
  const [downloadResults, setDownloadResults] = useState<Record<string, { success: boolean; message: string; source: string }>>({});
  const [batchDownloading, setBatchDownloading] = useState(false);

  const runAnalysis = useCallback(async (useDeep = false) => {
    if (!instanceDir) return;
    setLoading(true);
    if (useDeep) setDeepAnalyzing(true);
    try {
      if (useDeep) {
        const result = await invoke<any>("deep_analyze_with_api", {
          instanceDir,
          mcVersion: config.versionName || "1.20.1",
          loader: (config.loadName || "forge").toLowerCase().replace(/[^a-z]/g, ""),
        });
        setAnalysis(result);
        setIsDeepAnalysis(true);
      } else {
        const result = await invoke<any>("get_mod_dependencies_analysis", { instanceDir });
        setAnalysis(result);
        setIsDeepAnalysis(false);
      }
    } catch (e) {
      console.error("Mod dependency analysis failed:", e);
    } finally {
      setLoading(false);
      setDeepAnalyzing(false);
    }
  }, [instanceDir, config]);

  const refreshLoaderReport = useCallback(async () => {
    if (!instanceDir) return;
    try {
      const report = await invoke<LoaderLogReport>("analyze_loader_logs", { instanceDir });
      setLoaderReport(report);
    } catch (e) {
      console.error("Failed to analyze loader logs:", e);
    }
  }, [instanceDir]);

  useEffect(() => {
    if (!instanceLoading && instanceDir) {
      runAnalysis(false);
      refreshLoaderReport();
    }
  }, [instanceLoading, instanceDir, runAnalysis, refreshLoaderReport]);

  const totalIssues =
    (analysis?.missing_dependencies.length || 0) +
    (analysis?.version_mismatches.length || 0) +
    (analysis?.incompatible_mods.length || 0);

  const totalLoaderIssues = loaderReport?.errors.length || 0;

  const mcParams = useMemo(() => ({
    mcVersion: config.versionName || "1.20.1",
    loader: (config.loadName || "forge").toLowerCase().replace(/[^a-z]/g, ""),
  }), [config]);

  const markImmediatePass = useCallback((key: string, modId: string, extra: Record<string, { success: boolean; message: string; source: string }> = {}) => {
    setDownloadResults((prev) => ({
      ...prev,
      ...extra,
      [key]: {
        success: true,
        message: `模组已存在，通过校验 (${modId})`,
        source: "fastcheck",
      },
    }));
  }, []);

  const fastCheckModExists = useCallback(async (modId: string): Promise<boolean> => {
    if (!instanceDir || !modId) return false;
    try {
      return await invoke<boolean>("check_mod_installed", {
        instanceDir,
        modId,
      });
    } catch {
      return false;
    }
  }, [instanceDir]);

  const handleDownloadLoaderError = async (err: LoaderDependencyError) => {
    if (!instanceDir) return;
    const key = `loader_${err.mod_id}`;
    setDownloadResults((prev) => {
      const u = { ...prev };
      delete u[key];
      return u;
    });

    const alreadyInstalled = await fastCheckModExists(err.mod_id);
    if (alreadyInstalled) {
      markImmediatePass(key, err.mod_id);
      setTimeout(() => runAnalysis(isDeepAnalysis), 600);
      setTimeout(refreshLoaderReport, 1000);
      return;
    }

    setDownloadingMods((prev) => ({ ...prev, [key]: "pending" }));
    try {
      const results = await invoke<any[]>("auto_download_with_dependencies", {
        instanceDir,
        modId: err.mod_id,
        ...mcParams,
      });
      const newResults: Record<string, { success: boolean; message: string; source: string }> = {};
      results.forEach((r: any) => {
        newResults[r.mod_id] = { success: r.success, message: r.message, source: r.source };
      });
      newResults[key] = {
        success: results.some((r: any) => r.success),
        message: results.map((r: any) => r.message).join("；"),
        source: "loader_based",
      };
      setDownloadResults((prev) => ({ ...prev, ...newResults }));
      setTimeout(() => runAnalysis(isDeepAnalysis), 1500);
      setTimeout(refreshLoaderReport, 2000);
    } catch (e: any) {
      setDownloadResults((prev) => ({
        ...prev,
        [key]: { success: false, message: e?.toString() || "失败", source: "error" },
      }));
    } finally {
      setDownloadingMods((prev) => {
        const u = { ...prev };
        delete u[key];
        return u;
      });
    }
  };

  const handleBatchDownloadLoaderErrors = async () => {
    if (!instanceDir || !loaderReport?.errors.length) return;
    setBatchDownloading(true);
    const ids: string[] = loaderReport.errors
      .map((e) => e.mod_id)
      .filter((m) => m && !is_platform_dep(m));
    const deduped = Array.from(new Set(ids));
    const allResults: Record<string, { success: boolean; message: string; source: string }> = {};
    for (const mid of deduped) {
      const key = `loader_${mid}`;
      const alreadyInstalled = await fastCheckModExists(mid);
      if (alreadyInstalled) {
        allResults[key] = {
          success: true,
          message: `模组已存在，通过校验 (${mid})`,
          source: "fastcheck",
        };
        continue;
      }
      setDownloadingMods((prev) => ({ ...prev, [key]: "pending" }));
      try {
        const results = await invoke<any[]>("auto_download_with_dependencies", {
          instanceDir,
          modId: mid,
          ...mcParams,
        });
        results.forEach((r: any) => {
          allResults[r.mod_id] = {
            success: r.success,
            message: r.message,
            source: r.source,
          };
        });
      } catch (e: any) {
        allResults[key] = {
          success: false,
          message: e?.toString() || "失败",
          source: "error",
        };
      } finally {
        setDownloadingMods((prev) => {
          const u = { ...prev };
          delete u[key];
          return u;
        });
      }
    }
    setDownloadResults((prev) => ({ ...prev, ...allResults }));
    setTimeout(() => runAnalysis(isDeepAnalysis), 1500);
    setTimeout(refreshLoaderReport, 2000);
    setBatchDownloading(false);
  };

  const handleDownloadSingle = async (modId: string) => {
    if (!instanceDir) return;
    setDownloadResults((prev) => {
      const updated = { ...prev };
      delete updated[modId];
      return updated;
    });
    const alreadyInstalled = await fastCheckModExists(modId);
    if (alreadyInstalled) {
      markImmediatePass(modId, modId);
      setTimeout(() => runAnalysis(isDeepAnalysis), 600);
      setTimeout(refreshLoaderReport, 1000);
      return;
    }
    setDownloadingMods((prev) => ({ ...prev, [modId]: "pending" }));
    try {
      const result = await invoke<any>("auto_download_missing_dependency", {
        instanceDir,
        modId,
        ...mcParams,
      });
      setDownloadResults((prev) => ({
        ...prev,
        [modId]: {
          success: result.success,
          message: result.message,
          source: result.source,
        },
      }));
      if (result.success) {
        setTimeout(() => runAnalysis(isDeepAnalysis), 1000);
        setTimeout(refreshLoaderReport, 1500);
      }
    } catch (error: any) {
      setDownloadResults((prev) => ({
        ...prev,
        [modId]: {
          success: false,
          message: error?.toString() || "下载失败",
          source: "error",
        },
      }));
    } finally {
      setDownloadingMods((prev) => {
        const updated = { ...prev };
        delete updated[modId];
        return updated;
      });
    }
  };

  const handleDownloadWithDeps = async (modId: string) => {
    if (!instanceDir) return;
    const alreadyInstalled = await fastCheckModExists(modId);
    if (alreadyInstalled) {
      markImmediatePass(modId, modId);
      setTimeout(() => runAnalysis(isDeepAnalysis), 600);
      setTimeout(refreshLoaderReport, 1000);
      return;
    }
    setDownloadingMods((prev) => ({ ...prev, [modId]: "pending" }));
    try {
      const results = await invoke<any[]>("auto_download_with_dependencies", {
        instanceDir,
        modId,
        ...mcParams,
      });
      const newResults: Record<string, { success: boolean; message: string; source: string }> = {};
      results.forEach((r: any) => {
        newResults[r.mod_id] = { success: r.success, message: r.message, source: r.source };
      });
      setDownloadResults((prev) => ({ ...prev, ...newResults }));
      setTimeout(() => runAnalysis(isDeepAnalysis), 1500);
      setTimeout(refreshLoaderReport, 2000);
    } catch (error: any) {
      setDownloadResults((prev) => ({
        ...prev,
        [modId]: { success: false, message: error?.toString() || "失败", source: "error" },
      }));
    } finally {
      setDownloadingMods((prev) => {
        const u = { ...prev };
        delete u[modId];
        return u;
      });
    }
  };

  const handleBatchDownload = async () => {
    if (!instanceDir || !analysis?.missing_dependencies.length) return;
    setBatchDownloading(true);
    const missingIds = analysis.missing_dependencies.map((d) => d.mod_id);
    try {
      const allResults: Record<string, { success: boolean; message: string; source: string }> = {};
      for (const mid of missingIds) {
        const alreadyInstalled = await fastCheckModExists(mid);
        if (alreadyInstalled) {
          allResults[mid] = {
            success: true,
            message: `模组已存在，通过校验 (${mid})`,
            source: "fastcheck",
          };
          continue;
        }
        setDownloadingMods((prev) => ({ ...prev, [mid]: "pending" }));
        try {
          const results = await invoke<any[]>("auto_download_with_dependencies", {
            instanceDir,
            modId: mid,
            ...mcParams,
          });
          results.forEach((r: any) => {
            allResults[r.mod_id] = {
              success: r.success,
              message: r.message,
              source: r.source,
            };
          });
        } catch (e: any) {
          allResults[mid] = {
            success: false,
            message: e?.toString() || "失败",
            source: "error",
          };
        } finally {
          setDownloadingMods((prev) => {
            const u = { ...prev };
            delete u[mid];
            return u;
          });
        }
      }
      setDownloadResults((prev) => ({ ...prev, ...allResults }));
      setTimeout(() => runAnalysis(isDeepAnalysis), 1500);
      setTimeout(refreshLoaderReport, 2000);
    } catch (error) {
      console.error("批量下载失败:", error);
    } finally {
      setBatchDownloading(false);
    }
  };

  return (
    <Card className="flex flex-col min-h-0">
      <CardHeader className="flex-row items-center justify-between py-3">
        <CardTitle className="flex items-center gap-2 text-sm">
          <Puzzle className="size-4 text-emerald-500" />
          {t("launch.modDeps.title")}
          {analysis && (
            <Badge
              variant={totalIssues > 0 ? "destructive" : "secondary"}
              className="text-[10px] ml-1"
            >
              {totalIssues > 0 ? `${totalIssues} ${t("launch.modDeps.issues")}` : t("launch.modDeps.allOk")}
            </Badge>
          )}
        </CardTitle>
        <div className="flex items-center gap-1">
          {isDeepAnalysis && (
            <Badge variant="default" className="bg-green-500 text-white text-[10px]">
              {t("launch.modDeps.apiMode")}
            </Badge>
          )}
          <Button
            variant="outline"
            size="sm"
            onClick={() => runAnalysis(true)}
            disabled={deepAnalyzing}
            className="h-7 text-xs"
          >
            <AlertTriangle className="size-3 mr-1" />
            {deepAnalyzing ? t("launch.modDeps.deepAnalyzing") : t("launch.modDeps.deepAnalysis")}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 w-7 p-0"
            onClick={() => runAnalysis(false)}
            disabled={loading}
          >
            <RefreshCw className={`size-3.5 ${loading ? "animate-spin" : ""}`} />
          </Button>
        </div>
      </CardHeader>
      <CardContent className="py-0 px-4 pb-3">
        {loading && !analysis ? (
          <div className="flex items-center justify-center py-4 text-xs text-muted-foreground">
            <RefreshCw className="size-4 animate-spin mr-2" />
            {t("launch.modDeps.analyzing")}
          </div>
        ) : !analysis ? (
          <p className="text-xs text-muted-foreground py-2">{t("launch.modDeps.noInstance")}</p>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center justify-between text-xs">
              <div className="flex items-center gap-3">
                <span className="flex items-center gap-1">
                  <Package className="size-3" />
                  {analysis.total_mods} {t("launch.modDeps.mods")}
                </span>
                {analysis.all_resolved ? (
                  <span className="flex items-center gap-1 text-green-500">
                    <CheckCircle className="size-3" />
                    {t("launch.modDeps.allResolved")}
                  </span>
                ) : (
                  <span className="flex items-center gap-1 text-amber-500">
                    <AlertTriangle className="size-3" />
                    {totalIssues} {t("launch.modDeps.issues")}
                  </span>
                )}
              </div>
            </div>

            {loaderReport && loaderReport.errors.length > 0 && (
              <div className="space-y-1.5 p-2 rounded-lg border border-destructive/30 bg-destructive/5">
                <div className="flex items-center justify-between">
                  <div className="text-[11px] font-medium text-destructive flex items-center gap-1">
                    <XCircle className="size-3" />
                    启动日志报告的真实问题 · 高可信度 ({loaderReport.errors.length})
                  </div>
                  {loaderReport.errors.some((e) => !is_platform_dep(e.mod_id)) && (
                    <Button
                      size="sm"
                      onClick={handleBatchDownloadLoaderErrors}
                      disabled={batchDownloading}
                      className="h-6 text-[11px] bg-destructive hover:bg-destructive/90 text-white"
                    >
                      {batchDownloading ? (
                        <RefreshCw className="size-3 mr-1 animate-spin" />
                      ) : (
                        <Download className="size-3 mr-1" />
                      )}
                      按日志修复
                    </Button>
                  )}
                </div>
                <div className="max-h-36 overflow-y-auto space-y-1 pr-1">
                  {loaderReport.errors.map((err, i) => {
                    const actionKey = `loader_${err.mod_id}`;
                    const isDL = !!downloadingMods[actionKey];
                    const result = downloadResults[actionKey] ||
                      (err.mod_id && downloadResults[err.mod_id]);
                    const canFix = err.mod_id && !is_platform_dep(err.mod_id) && err.error_type !== "incompatible";
                    const errBadge =
                      err.error_type === "missing"
                        ? { txt: "缺失", cls: "bg-red-500/20 text-red-700 dark:text-red-300" }
                        : err.error_type === "version_mismatch"
                          ? { txt: "版本不对", cls: "bg-amber-500/20 text-amber-700 dark:text-amber-300" }
                          : { txt: "不兼容", cls: "bg-purple-500/20 text-purple-700 dark:text-purple-300" };
                    return (
                      <div key={i} className="p-2 rounded bg-destructive/5 border border-destructive/10">
                        <div className="flex items-center justify-between gap-2">
                          <div className="flex items-center gap-1.5 min-w-0 flex-1">
                            <Badge className={cn("text-[9px] h-4 px-1.5 border-0", errBadge.cls)} variant="secondary">
                              {errBadge.txt}
                            </Badge>
                            {err.mod_id && (
                              <span className="font-mono text-[11px] font-semibold text-foreground truncate">
                                {err.mod_id}
                              </span>
                            )}
                            {err.required_version && (
                              <span className="text-[10px] text-muted-foreground shrink-0">
                                需要 {err.required_version}
                              </span>
                            )}
                            {err.required_by && (
                              <span className="text-[10px] text-muted-foreground truncate">
                                · 来自 {err.required_by}
                              </span>
                            )}
                          </div>
                          {canFix && (
                            <div className="flex items-center gap-1 shrink-0">
                              <Button
                                size="sm"
                                variant="outline"
                                className="h-5 text-[10px] px-2 border-destructive/40 text-destructive hover:bg-destructive/10"
                                onClick={() => handleDownloadLoaderError(err)}
                                disabled={isDL || !!result}
                                title="根据日志里报告的模组ID搜索安装（含依赖链）"
                              >
                                {isDL ? (
                                  <RefreshCw className="size-2.5 animate-spin mr-1" />
                                ) : (
                                  <Package className="size-2.5 mr-1" />
                                )}
                                修复
                              </Button>
                            </div>
                          )}
                        </div>
                        <div className="text-[10px] text-destructive/80 mt-1 whitespace-pre-wrap break-all">
                          {err.message}
                        </div>
                        {result && (
                          <div className={`text-[10px] mt-1 ${result.success ? "text-green-600" : "text-destructive"}`}>
                            {result.message}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}

            <div className="space-y-1.5 p-2 rounded-lg border border-border/50 bg-muted/20">
              <div className="flex items-center justify-between">
                <div className="text-[11px] font-medium text-muted-foreground flex items-center gap-1">
                  <Info className="size-3" />
                  模组作者声明的依赖 · 参考信息
                </div>
                {analysis.missing_dependencies.length > 0 && (
                  <Button
                    size="sm"
                    onClick={handleBatchDownload}
                    disabled={batchDownloading}
                    className="h-6 text-xs bg-amber-600 hover:bg-amber-600/90 text-white"
                  >
                    {batchDownloading ? (
                      <RefreshCw className="size-3 mr-1 animate-spin" />
                    ) : (
                      <Download className="size-3 mr-1" />
                    )}
                    {t("launch.modDeps.batchFix")}
                  </Button>
                )}
              </div>

              {analysis.missing_dependencies.length > 0 && (
                <div className="space-y-1.5">
                  <div className="text-[11px] font-medium text-amber-600 flex items-center gap-1 pt-1">
                    <XCircle className="size-3" />
                    {t("launch.modDeps.missingRequired")} ({analysis.missing_dependencies.length})
                  </div>
                  <div className="max-h-28 overflow-y-auto space-y-1 pr-1">
                    {analysis.missing_dependencies.map((dep, i) => {
                      const isDL = !!downloadingMods[dep.mod_id];
                      const result = downloadResults[dep.mod_id];
                      return (
                        <div key={i} className="flex items-center justify-between p-1.5 rounded bg-amber-500/10 text-xs">
                          <div className="min-w-0 flex-1">
                            <div className="font-medium truncate">{dep.mod_id}</div>
                            <div className="text-[10px] text-muted-foreground truncate">
                              {t("launch.modDeps.requiredBy")}: {dep.required_by.join(", ")}
                            </div>
                            {result && (
                              <div className={`text-[10px] ${result.success ? "text-green-600" : "text-destructive"}`}>
                                {result.message}
                              </div>
                            )}
                          </div>
                          <div className="flex items-center gap-1 shrink-0">
                            <Button
                              size="sm"
                              variant="outline"
                              className="h-5 text-[10px] px-2"
                              onClick={() => handleDownloadWithDeps(dep.mod_id)}
                              disabled={isDL || !!result}
                            >
                              {isDL ? (
                                <RefreshCw className="size-2.5 animate-spin" />
                              ) : (
                                <Package className="size-2.5" />
                              )}
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              className="h-5 text-[10px] px-1"
                              onClick={() => handleDownloadSingle(dep.mod_id)}
                              disabled={isDL || !!result}
                            >
                              {t("launch.modDeps.single")}
                            </Button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {analysis.version_mismatches.length > 0 && (
                <div className="space-y-1.5 pt-1">
                  <div className="text-[11px] font-medium text-amber-500 flex items-center gap-1">
                    <AlertTriangle className="size-3" />
                    {t("launch.modDeps.versionMismatch")} ({analysis.version_mismatches.length})
                  </div>
                  <div className="max-h-20 overflow-y-auto space-y-1 pr-1">
                    {analysis.version_mismatches.map((dep, i) => {
                      const updateKey = dep.mod_id + "_update";
                      const isUpdating = !!downloadingMods[updateKey];
                      const result = downloadResults[updateKey];
                      return (
                        <div key={i} className="flex items-center justify-between p-1.5 rounded bg-amber-500/5 text-xs">
                          <div className="min-w-0 flex-1">
                            <div className="font-medium truncate">{dep.mod_id}</div>
                            <div className="text-[10px] text-muted-foreground">
                              {dep.version} → <span className="text-green-600">{dep.recommended_version}</span>
                            </div>
                          </div>
                          <Button
                            size="sm"
                            variant="outline"
                            className="h-5 text-[10px] px-2 shrink-0"
                            onClick={() => handleDownloadSingle(dep.mod_id)}
                            disabled={isUpdating || !!result}
                          >
                            {isUpdating ? (
                              <RefreshCw className="size-2.5 animate-spin" />
                            ) : (
                              <Download className="size-2.5" />
                            )}
                          </Button>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {analysis.incompatible_mods.length > 0 && (
                <div className="space-y-1 pt-1">
                  <div className="text-[11px] font-medium text-purple-600 flex items-center gap-1">
                    <AlertTriangle className="size-3" />
                    声明不兼容 ({analysis.incompatible_mods.length})
                  </div>
                  <div className="max-h-16 overflow-y-auto space-y-0.5 pr-1 text-[11px] text-purple-700 dark:text-purple-300">
                    {analysis.incompatible_mods.map((dep, i) => (
                      <div key={i}>
                        · <b>{dep.mod_id}</b> ↔ {dep.required_by.join(", ")}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {!analysis.all_resolved && (
              <p className="text-[10px] text-amber-600 dark:text-amber-400 flex items-start gap-1 pt-1 border-t border-border/50">
                <Info className="size-3 mt-0.5 shrink-0" />
                {t("launch.modDeps.scanNote")}
              </p>
            )}

            {analysis.all_resolved && totalIssues === 0 && totalLoaderIssues === 0 && logs.length > 0 && (
              <div className="flex items-center gap-2 py-1 text-xs text-green-500">
                <CheckCircle className="size-3.5" />
                {t("launch.modDeps.allSatisfied")}
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}