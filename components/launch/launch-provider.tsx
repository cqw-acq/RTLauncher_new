"use client";

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAccountContext } from "@/components/accounts/account-provider";
import { isTauriRuntime } from "@/lib/tauri-runtime";
import { log4jParser } from "@/components/launch/log4j-progress-parser";
import { useI18n } from "@/components/i18n/use-i18n";
import { analyzeLaunchLogs } from "@/components/launch/launch-analyzer";
import type {
  LaunchAnalysisReport,
  LaunchConfig,
  LaunchLogEntry,
  LaunchProgress,
  LaunchStatus,
} from "@/types";

/** 默认启动配置 */
export const DEFAULT_INITIAL_JVM_ARGS = `-XX:+UnlockExperimentalVMOptions
-XX:+AlwaysPreTouch
-XX:+DisableExplicitGC
-XX:MaxGCPauseMillis=200
-Dfml.ignorePatchDiscrepancies=true
-Dfml.ignoreInvalidMinecraftCertificates=true
-Duser.language=zh
-Duser.country=CN
-Dminecraft.api.env=production
-Dminecraft.api.location=https://api.minecraftservices.com/
-Dfml.readTimeout=180
-Dio.netty.allocator.type=unpooled`;

const DEFAULT_LAUNCH_CONFIG: LaunchConfig = {
  minecraftPath: "",
  javaPath: "",
  wrapperPath: "",
  maxMemory: "4096",
  versionName: "",
  loadType: "0",
  loadName: "",
  playerName: "",
  authToken: "",
  uuid: "",
  windowWidth: "873",
  windowHeight: "486",
  authlibInjectorPath: "",
  yggdrasilApi: "",
  prefetchedData: "",
  customJvmArgs: DEFAULT_INITIAL_JVM_ARGS,
};

interface LaunchContextValue {
  config: LaunchConfig;
  updateConfig: (patch: Partial<LaunchConfig>) => void;
  status: LaunchStatus;
  logs: LaunchLogEntry[];
  errorMessage: string | null;
  launchGame: (overrides?: Partial<LaunchConfig>) => Promise<void>;
  cancelLaunch: () => Promise<void>;
  clearLogs: () => void;
  lastCommandArgs: string | null;
  lastLaunchTime: string | null;
  configLoaded: boolean;
  progress: LaunchProgress | null;
  launchStartedAt: number | null;
  launchEndedAt: number | null;
  lastExitCode: number | null;
  generateReport: () => LaunchAnalysisReport;
  exportLaunchReport: () => Promise<string>;
}

const LaunchContext = createContext<LaunchContextValue | null>(null);

export function useLaunchContext() {
  const ctx = useContext(LaunchContext);
  if (!ctx) {
    throw new Error("useLaunchContext must be used within LaunchProvider");
  }
  return ctx;
}

export function LaunchProvider({ children }: { children: React.ReactNode }) {
  const { t, language } = useI18n();
  const [config, setConfig] = useState<LaunchConfig>(DEFAULT_LAUNCH_CONFIG);
  const [configLoaded, setConfigLoaded] = useState(false);
  const [status, setStatus] = useState<LaunchStatus>("idle");
  const [logs, setLogs] = useState<LaunchLogEntry[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [lastCommandArgs, setLastCommandArgs] = useState<string | null>(null);
  const [lastLaunchTime, setLastLaunchTime] = useState<string | null>(null);
  const [progress, setProgress] = useState<LaunchProgress | null>(null);
  const [launchStartedAt, setLaunchStartedAt] = useState<number | null>(null);
  const [launchEndedAt, setLaunchEndedAt] = useState<number | null>(null);
  const [lastExitCode, setLastExitCode] = useState<number | null>(null);
  const logIdRef = useRef(0);


  // 本地配置先恢复，使首屏不必等待原生 I/O；原生路径查询完成后再无缝合并。
  useEffect(() => {
    let cancelled = false;
    let savedConfig: Partial<LaunchConfig> = {};
    try {
      const saved = localStorage.getItem("rtl-launch-config");
      if (saved) savedConfig = JSON.parse(saved);
      const savedTime = localStorage.getItem("rtl-last-launch-time");
      if (savedTime) setLastLaunchTime(savedTime);
    } catch { /* ignore */ }

    let nativePathsResolved = false;
    let localConfigApplied = false;

    const tryMarkLoaded = () => {
      // 两个阶段都完成后才标记 configLoaded=true：
      // 1) localStorage 配置应用
      // 2) Tauri 原生返回的路径配置应用（或失败也视为完成）
      if (!cancelled && localConfigApplied && nativePathsResolved) {
        setConfigLoaded(true);
      }
    };

    queueMicrotask(() => {
      if (cancelled) return;
      setConfig((prev) => ({ ...prev, ...savedConfig }));
      localConfigApplied = true;
      tryMarkLoaded();
    });

    void invoke<{
      selected_java_path: string;
      selected_minecraft_path: string;
    }>("get_launcher_paths_config")
      .then((pathsCfg) => {
        if (cancelled) return;
        setConfig((prev) => ({
          ...prev,
          ...(pathsCfg.selected_java_path ? { javaPath: pathsCfg.selected_java_path } : {}),
          ...(pathsCfg.selected_minecraft_path ? { minecraftPath: pathsCfg.selected_minecraft_path } : {}),
        }));
      })
      .catch(() => {})
      .finally(() => {
        nativePathsResolved = true;
        tryMarkLoaded();
      });
    return () => { cancelled = true; };
  }, []);

  const { selectedProfile } = useAccountContext();

  const MAX_LOG_ENTRIES = 500;

  const addLog = useCallback(
    (level: LaunchLogEntry["level"], message: string) => {
      const entry: LaunchLogEntry = {
        id: ++logIdRef.current,
        timestamp: new Date().toLocaleTimeString(),
        level,
        message,
      };
      setLogs((prev) => {
        const next = [...prev, entry];
        return next.length > MAX_LOG_ENTRIES ? next.slice(-MAX_LOG_ENTRIES) : next;
      });
    },
    []
  );

  const updateConfig = useCallback((patch: Partial<LaunchConfig>) => {
    setConfig((prev) => {
      const next = { ...prev, ...patch };
      // 持久化到 localStorage
      try {
        localStorage.setItem("rtl-launch-config", JSON.stringify(next));
      } catch {
        // ignore
      }
      return next;
    });
  }, []);

  const clearLogs = useCallback(() => {
    setLogs([]);
    logIdRef.current = 0;
  }, []);

  // 监听游戏日志事件（来自 Minecraft log4j stdout/stderr）
  useEffect(() => {
    if (!isTauriRuntime()) return;

    let unlisten: (() => void) | null = null;
    listen<{ level: string; message: string }>("game-log", (event) => {
      const { level, message } = event.payload;
      const logLevel: "error" | "info" | "warn" =
        level === "error" || level === "warn" ? level : "info";

      // 使用 log4j 解析器分析日志并更新进度
      if (status === "launching" || status === "preparing") {
        const parsedProgress = log4jParser.parseLog(message);
        if (parsedProgress.stage) {
          const allStages = log4jParser.getAllStages();
          const currentStageIndex = allStages.findIndex(s => s.id === parsedProgress.stage?.id);
          setProgress({
            currentStep: currentStageIndex + 1,
            totalSteps: allStages.length,
            currentStage: parsedProgress.stage.name,
            percentage: parsedProgress.progress,
          });
        }
      }

      setLogs((prev) => {
        const next = [
          ...prev,
          {
            id: ++logIdRef.current,
            timestamp: new Date().toLocaleTimeString(),
            level: logLevel,
            message,
          },
        ];
        return next.length > MAX_LOG_ENTRIES ? next.slice(-MAX_LOG_ENTRIES) : next;
      });
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [status]);

  // 监听游戏进程退出事件
  useEffect(() => {
    if (!isTauriRuntime()) return;

    let unlisten: (() => void) | null = null;
    listen<number>("game-exited", (event) => {
      const exitCode = event.payload;
      const now = Date.now();
      const timeStr = new Date(now).toLocaleString();
      setLastLaunchTime(timeStr);
      setLaunchEndedAt(now);
      setLastExitCode(exitCode);
      try { localStorage.setItem("rtl-last-launch-time", timeStr); } catch { /* ignore */ }
      setStatus(exitCode === 0 ? "stopped" : "error");
      setProgress(null); // 清理进度状态
      log4jParser.reset(); // 重置日志解析器
      setLogs((prev) => [
        ...prev,
        {
          id: ++logIdRef.current,
          timestamp: new Date(now).toLocaleTimeString(),
          level: exitCode === 0 ? "info" : "warn",
          message: t("launch.provider.gameExitedWithCodeExitCode", { exitCode: exitCode }),
        },
      ]);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [t]);

  // 监听游戏完全启动事件（JVM 启动完成、资源加载完成）
  useEffect(() => {
    if (!isTauriRuntime()) return;

    let unlisten: (() => void) | null = null;
    listen<number>("game-fully-started", (event) => {
      const pid = event.payload;
      setStatus("running");
      setProgress(null);
      setLogs((prev) => [
        ...prev,
        {
          id: ++logIdRef.current,
          timestamp: new Date().toLocaleTimeString(),
          level: "info",
          message: t("launch.provider.gameFullyStartedPidPidStoppedJvmTracking", { pid: pid }),
        },
      ]);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [t]);

  // 监听启动进度事件
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<{ current_step: number; total_steps: number; current_stage: string; percentage: number }>("launch-progress", (event) => {
      const { current_step, total_steps, current_stage, percentage } = event.payload;
      setProgress({
        currentStep: current_step,
        totalSteps: total_steps,
        currentStage: current_stage,
        percentage: percentage,
      });
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  const cancelLaunch = useCallback(
    async () => {
      if (status !== "preparing" && status !== "launching" && status !== "running") {
        return;
      }
      try {
        const result = await invoke<string>("kill_game_process");
        setLogs((prev) => [
          ...prev,
          {
            id: ++logIdRef.current,
            timestamp: new Date().toLocaleTimeString(),
            level: "warn",
            message: result,
          },
        ]);
        setStatus("idle");
        setProgress(null);
      } catch (e) {
        setErrorMessage(e instanceof Error ? e.message : String(e));
      }
    },
    [status]
  );

  const launchGame = useCallback(
    async (overrides?: Partial<LaunchConfig>) => {
      const merged = { ...config, ...overrides };

      // 校验必要参数
      if (!merged.minecraftPath) {
        setErrorMessage(t("launch.provider.setTheMinecraftGameDirectory"));
        return;
      }
      if (!merged.javaPath) {
        setErrorMessage(t("launch.provider.setAJavaPath"));
        return;
      }
      if (!merged.versionName) {
        setErrorMessage(t("launch.provider.selectAGameVersion"));
        return;
      }
      if (!selectedProfile) {
        setErrorMessage(t("launch.provider.selectAPlayerAccountFirst"));
        return;
      }

      setErrorMessage(null);
      setProgress(null);
      log4jParser.reset(); // 重置日志解析器
      const now = Date.now();
      setLaunchStartedAt(now);
      setLaunchEndedAt(null);
      setLastExitCode(null);
      setStatus("preparing");
      addLog("info", t("launch.provider.preparingLaunchArguments"));

      try {
        setStatus("launching");
        addLog("info", t("launch.provider.launchVersionVersionName", { versionName: merged.versionName }));
        addLog("info", t("launch.provider.playerName", { name: selectedProfile.name }));
        addLog("info", t("launch.provider.maximumMemoryMaxMemoryMb", { maxMemory: merged.maxMemory }));

        if (merged.loadType !== "0") {
          addLog("info", t("launch.provider.loaderLoadName", { loadName: merged.loadName }));
        }

        const result = await invoke<string>("launch_game", {
          minecraftPath: merged.minecraftPath,
          javaPath: merged.javaPath,
          wrapperPath: merged.wrapperPath,
          maxMemory: merged.maxMemory,
          versionName: merged.versionName,
          playerName: merged.playerName || selectedProfile.name,
          authToken: merged.authToken || selectedProfile.accessToken || "",
          uuid: merged.uuid || selectedProfile.uuid || selectedProfile.id,
          authlibInjectorPath: merged.authlibInjectorPath,
          yggdrasilApi: merged.yggdrasilApi || selectedProfile.yggdrasilUrl || "",
          prefetchedData: merged.prefetchedData,
          loadType: merged.loadType,
          loadName: merged.loadName,
          windowWidth: merged.windowWidth || "873",
          windowHeight: merged.windowHeight || "486",
          customJvmArgs: merged.customJvmArgs || "",
        });

        setLastCommandArgs(result);
        setStatus("running");
        addLog("info", t("launch.provider.gameLaunched"));
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        const endedAt = Date.now();
        setLaunchEndedAt(endedAt);
        setStatus("error");
        setErrorMessage(msg);
        addLog("error", `${t("launch.provider.launchFailed")}: ${msg}`);
      }
    },
    [config, selectedProfile, addLog, t]
  );

  const generateReport = useCallback<() => LaunchAnalysisReport>(
    () => {
      const finalStatus: LaunchAnalysisReport["finalStatus"] =
        status === "running"
          ? "running"
          : status === "stopped"
            ? "stopped"
            : status === "error"
              ? "error"
              : status === "launching" || status === "preparing"
                ? "in_progress"
                : "idle";
      const report = analyzeLaunchLogs(logs, {
        language,
        startedAt: launchStartedAt,
        endedAt: launchEndedAt,
        exitCode: lastExitCode,
        finalStatus,
        accountType: selectedProfile?.authType,
      });
      return {
        ...report,
        launchParameters: lastCommandArgs ?? undefined,
      };
    },
    [logs, language, launchStartedAt, launchEndedAt, lastExitCode, status, selectedProfile, lastCommandArgs],
  );

  const exportLaunchReport = useCallback(async () => {
    try {
      const report = generateReport();
      const reportJson = JSON.stringify(report, null, 2);
      return await invoke<string>("export_launch_report", {
        minecraftPath: config.minecraftPath,
        versionName: config.versionName,
        launchParameters: lastCommandArgs ?? "",
        accountType: selectedProfile?.authType ?? "offline",
        reportJson,
      });
    } catch (err) {
      console.error("导出启动报告失败:", err);
      throw err;
    }
  }, [config, lastCommandArgs, selectedProfile, generateReport]);

  return (
    <LaunchContext.Provider
      value={{
        config,
        updateConfig,
        status,
        logs,
        errorMessage,
        launchGame,
        cancelLaunch,
        clearLogs,
        lastCommandArgs,
        lastLaunchTime,
        configLoaded,
        progress,
        launchStartedAt,
        launchEndedAt,
        lastExitCode,
        generateReport,
        exportLaunchReport,
      }}
    >
      {children}
    </LaunchContext.Provider>
  );
}