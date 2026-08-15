"use client";

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from "react";

import {
  tauriMultiplayerApi,
  type MultiplayerApi,
  type MultiplayerPaths,
} from "@/components/multiplayer/multiplayer-api";
import {
  initialMultiplayerState,
  multiplayerReducer,
  type MultiplayerState,
} from "@/components/multiplayer/multiplayer-state";

type MultiplayerContextValue = MultiplayerState & {
  checkStatus: () => Promise<void>;
  installOpenP2P: (srcPath: string) => Promise<string>;
  startAsHost: (roomName: string, port: string) => Promise<string>;
  startAsJoin: (encodedInfo: string, playerName: string) => Promise<string>;
  stopOpenP2P: () => Promise<void>;
  isRunning: () => Promise<boolean>;
  pollLog: () => Promise<string>;
  clearLog: () => void;
  getOpenP2PPaths: () => Promise<MultiplayerPaths>;
  getOpenP2PDir: () => Promise<string>;
  getOpenP2PPath: () => Promise<string>;
};

const MultiplayerContext = createContext<MultiplayerContextValue | null>(null);

export function useMultiplayerContext() {
  const context = useContext(MultiplayerContext);
  if (!context) {
    throw new Error("useMultiplayerContext must be used within MultiplayerProvider");
  }
  return context;
}

function errorMessage(error: unknown, fallback: string) {
  if (typeof error === "string") return error;
  if (error instanceof Error && error.message) return error.message;
  return fallback;
}

type MultiplayerProviderProps = {
  children: ReactNode;
  api?: MultiplayerApi;
};

export function MultiplayerProvider({
  children,
  api = tauriMultiplayerApi,
}: MultiplayerProviderProps) {
  const [state, dispatch] = useReducer(multiplayerReducer, initialMultiplayerState);
  const logRequestRef = useRef<Promise<string> | null>(null);

  const checkStatus = useCallback(async () => {
    try {
      const snapshot = await api.readStatus();
      dispatch({ type: "statusResolved", ...snapshot });
    } catch (error) {
      dispatch({ type: "failed", message: errorMessage(error, "状态检查失败") });
    }
  }, [api]);

  const installOpenP2P = useCallback(
    async (srcPath: string) => {
      try {
        const destination = await api.install(srcPath);
        await checkStatus();
        return destination;
      } catch (error) {
        dispatch({ type: "failed", message: errorMessage(error, "安装失败") });
        throw error;
      }
    },
    [api, checkStatus]
  );

  const startAsHost = useCallback(
    async (roomName: string, port: string) => {
      dispatch({ type: "operationStarted", operation: "start" });
      try {
        const encodedInfo = await api.encodeRoomInfo(roomName, port);
        const path = await api.startHost(roomName);
        dispatch({ type: "started", mode: "host", roomInfo: encodedInfo });
        return path;
      } catch (error) {
        dispatch({ type: "failed", message: errorMessage(error, "启动失败") });
        throw error;
      }
    },
    [api]
  );

  const startAsJoin = useCallback(
    async (encodedInfo: string, playerName: string) => {
      dispatch({ type: "operationStarted", operation: "start" });
      try {
        const path = await api.startJoin(encodedInfo, playerName);
        dispatch({ type: "started", mode: "join", roomInfo: encodedInfo });
        return path;
      } catch (error) {
        dispatch({ type: "failed", message: errorMessage(error, "启动失败") });
        throw error;
      }
    },
    [api]
  );

  const stopOpenP2P = useCallback(async () => {
    dispatch({ type: "operationStarted", operation: "stop" });
    try {
      await api.stop();
      dispatch({ type: "stopped" });
    } catch (error) {
      dispatch({ type: "failed", message: errorMessage(error, "停止失败") });
    }
  }, [api]);

  const isRunning = useCallback(async () => {
    try {
      return await api.isRunning();
    } catch (error) {
      console.error("检查 openp2p 运行状态失败:", error);
      return false;
    }
  }, [api]);

  const pollLog = useCallback(() => {
    if (logRequestRef.current) return logRequestRef.current;

    const request = api
      .pollLog()
      .then((text) => {
        dispatch({ type: "logReceived", text });
        return text;
      })
      .catch((error) => {
        console.error("轮询 openp2p 日志失败:", error);
        return "";
      })
      .finally(() => {
        if (logRequestRef.current === request) logRequestRef.current = null;
      });
    logRequestRef.current = request;
    return request;
  }, [api]);

  const clearLog = useCallback(() => dispatch({ type: "logCleared" }), []);
  const getOpenP2PPaths = useCallback(async () => {
    try {
      return await api.getPaths();
    } catch (error) {
      console.error("获取 openp2p 路径失败:", error);
      return { directory: "", executable: "" };
    }
  }, [api]);
  const getOpenP2PDir = useCallback(
    async () => (await getOpenP2PPaths()).directory,
    [getOpenP2PPaths]
  );
  const getOpenP2PPath = useCallback(
    async () => (await getOpenP2PPaths()).executable,
    [getOpenP2PPaths]
  );

  const value = useMemo<MultiplayerContextValue>(
    () => ({
      ...state,
      checkStatus,
      installOpenP2P,
      startAsHost,
      startAsJoin,
      stopOpenP2P,
      isRunning,
      pollLog,
      clearLog,
      getOpenP2PPaths,
      getOpenP2PDir,
      getOpenP2PPath,
    }),
    [
      state,
      checkStatus,
      installOpenP2P,
      startAsHost,
      startAsJoin,
      stopOpenP2P,
      isRunning,
      pollLog,
      clearLog,
      getOpenP2PPaths,
      getOpenP2PDir,
      getOpenP2PPath,
    ]
  );

  return <MultiplayerContext.Provider value={value}>{children}</MultiplayerContext.Provider>;
}
