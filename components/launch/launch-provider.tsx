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
import type { LaunchConfig, LaunchLogEntry, LaunchStatus } from "@/types";

/** 默认启动配置 */
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
};

interface LaunchContextValue {
  /** 启动配置 */
  config: LaunchConfig;
  /** 更新启动配置 */
  updateConfig: (patch: Partial<LaunchConfig>) => void;
  /** 当前启动状态 */
  status: LaunchStatus;
  /** 启动日志 */
  logs: LaunchLogEntry[];
  /** 错误信息 */
  errorMessage: string | null;
  /** 启动游戏 */
  launchGame: (overrides?: Partial<LaunchConfig>) => Promise<void>;
  /** 清空日志 */
  clearLogs: () => void;
  /** 最后一次启动的完整命令参数（调试用） */
  lastCommandArgs: string | null;
  /** 上次启动时间 */
  lastLaunchTime: string | null;
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
  const [config, setConfig] = useState<LaunchConfig>(DEFAULT_LAUNCH_CONFIG);

  // 客户端挂载后从 localStorage 恢复配置，避免 SSR hydration 不匹配
  useEffect(() => {
    try {
      const saved = localStorage.getItem("rtl-launch-config");
      if (saved) {
        setConfig((prev) => ({ ...prev, ...JSON.parse(saved) }));
      }
      const savedTime = localStorage.getItem("rtl-last-launch-time");
      if (savedTime) setLastLaunchTime(savedTime);
    } catch {
      // ignore
    }
  }, []);

  const [status, setStatus] = useState<LaunchStatus>("idle");
  const [logs, setLogs] = useState<LaunchLogEntry[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [lastCommandArgs, setLastCommandArgs] = useState<string | null>(null);
  const [lastLaunchTime, setLastLaunchTime] = useState<string | null>(null);
  const logIdRef = useRef(0);

  const { selectedProfile } = useAccountContext();

  const addLog = useCallback(
    (level: LaunchLogEntry["level"], message: string) => {
      const entry: LaunchLogEntry = {
        id: ++logIdRef.current,
        timestamp: new Date().toLocaleTimeString(),
        level,
        message,
      };
      setLogs((prev) => [...prev, entry]);
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
    let unlisten: (() => void) | null = null;
    listen<{ level: string; message: string }>("game-log", (event) => {
      const { level, message } = event.payload;
      const logLevel =
        level === "error" || level === "warn" ? (level as "error" | "warn") : "info";
      setLogs((prev) => [
        ...prev,
        {
          id: ++logIdRef.current,
          timestamp: new Date().toLocaleTimeString(),
          level: logLevel,
          message,
        },
      ]);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  // 监听游戏进程退出事件
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<number>("game-exited", (event) => {
      const exitCode = event.payload;
      const timeStr = new Date().toLocaleString("zh-CN");
      setLastLaunchTime(timeStr);
      try { localStorage.setItem("rtl-last-launch-time", timeStr); } catch { /* ignore */ }
      setStatus("idle");
      setLogs((prev) => [
        ...prev,
        {
          id: ++logIdRef.current,
          timestamp: new Date().toLocaleTimeString(),
          level: exitCode === 0 ? "info" : "warn",
          message: `游戏已退出，退出码: ${exitCode}`,
        },
      ]);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  const launchGame = useCallback(
    async (overrides?: Partial<LaunchConfig>) => {
      const merged = { ...config, ...overrides };

      // 校验必要参数
      if (!merged.minecraftPath) {
        setErrorMessage("请设置 Minecraft 游戏目录");
        return;
      }
      if (!merged.javaPath) {
        setErrorMessage("请设置 Java 路径");
        return;
      }
      if (!merged.versionName) {
        setErrorMessage("请选择游戏版本");
        return;
      }
      if (!selectedProfile) {
        setErrorMessage("请先选择一个玩家账户");
        return;
      }

      setErrorMessage(null);
      setStatus("preparing");
      addLog("info", "正在准备启动参数...");

      try {
        setStatus("launching");
        addLog("info", `启动版本: ${merged.versionName}`);
        addLog("info", `玩家: ${selectedProfile.name}`);
        addLog("info", `最大内存: ${merged.maxMemory}MB`);

        if (merged.loadType !== "0") {
          addLog("info", `加载器: ${merged.loadName}`);
        }

        const result = await invoke<string>("build_jvm_arguments", {
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
        });

        setLastCommandArgs(result);
        setStatus("running");
        addLog("info", "游戏已启动！");
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setStatus("error");
        setErrorMessage(msg);
        addLog("error", `启动失败: ${msg}`);
      }
    },
    [config, selectedProfile, addLog]
  );

  return (
    <LaunchContext.Provider
      value={{
        config,
        updateConfig,
        status,
        logs,
        errorMessage,
        launchGame,
        clearLogs,
        lastCommandArgs,
        lastLaunchTime,
      }}
    >
      {children}
    </LaunchContext.Provider>
  );
}
