"use client";

import React, { createContext, useCallback, useContext, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { MultiplayerMode } from "@/types";

/** 生成随机 16 位 UUID（仅包含 a-z0-9） */
function generateRoomId(): string {
  const chars = "abcdefghijklmnopqrstuvwxyz0123456789";
  let result = "";
  const arr = new Uint8Array(16);
  crypto.getRandomValues(arr);
  for (let i = 0; i < 16; i++) {
    result += chars[arr[i] % chars.length];
  }
  return result;
}

type PanelStatus = "idle" | "loading" | "running" | "error";

type MultiplayerContextValue = {
  // ── Tab 切换 ──
  mode: MultiplayerMode;
  setMode: (mode: MultiplayerMode) => void;

  // ── 房主面板状态（持久化，切 Tab 不丢失）──
  hostRoomName: string;
  hostPort: string;
  setHostPort: (v: string) => void;
  hostJoinCode: string | null;
  hostStatus: PanelStatus;
  hostError: string | null;
  handleHostRoom: () => Promise<void>;
  handleHostDisconnect: () => Promise<void>;

  // ── 加入面板状态（持久化，切 Tab 不丢失）──
  joinCode: string;
  setJoinCode: (v: string) => void;
  joinPlayerName: string;
  setJoinPlayerName: (v: string) => void;
  joinStatus: PanelStatus;
  joinError: string | null;
  handleJoinRoom: () => Promise<void>;
  handleJoinDisconnect: () => Promise<void>;

  // ── 工具 ──
  installOpenp2p: (srcPath: string) => Promise<string>;
};

const MultiplayerContext = createContext<MultiplayerContextValue | null>(null);

export function useMultiplayerContext() {
  const ctx = useContext(MultiplayerContext);
  if (!ctx) {
    throw new Error("useMultiplayerContext must be used within MultiplayerProvider");
  }
  return ctx;
}

export function MultiplayerProvider({ children }: { children: React.ReactNode }) {
  const [mode, setMode] = useState<MultiplayerMode>("host");

  // ── 房主面板 ──
  const hostRoomNameRef = useRef(generateRoomId());
  const [hostPort, setHostPort] = useState("25565");
  const [hostJoinCode, setHostJoinCode] = useState<string | null>(null);
  const [hostStatus, setHostStatus] = useState<PanelStatus>("idle");
  const [hostError, setHostError] = useState<string | null>(null);

  const handleHostRoom = useCallback(async () => {
    if (!hostPort.trim()) return;
    const roomName = hostRoomNameRef.current;
    setHostStatus("loading");
    setHostError(null);
    try {
      const code = await invoke<string>("mp_host_room", {
        roomName,
        port: hostPort.trim(),
      });
      setHostJoinCode(code);
      setHostStatus("running");
    } catch (e) {
      setHostStatus("error");
      setHostError(typeof e === "string" ? e : (e as Error).message ?? "启动失败");
    }
  }, [hostPort]);

  const handleHostDisconnect = useCallback(async () => {
    try { await invoke<void>("mp_disconnect"); } catch { /* ignore */ }
    hostRoomNameRef.current = generateRoomId(); // 断开后重新生成
    setHostStatus("idle");
    setHostJoinCode(null);
    setHostError(null);
  }, []);

  // ── 加入面板 ──
  const [joinCode, setJoinCode] = useState("");
  const [joinPlayerName, setJoinPlayerName] = useState("");
  const [joinStatus, setJoinStatus] = useState<PanelStatus>("idle");
  const [joinError, setJoinError] = useState<string | null>(null);

  const handleJoinRoom = useCallback(async () => {
    if (!joinCode.trim() || !joinPlayerName.trim()) return;
    setJoinStatus("loading");
    setJoinError(null);
    try {
      await invoke<void>("mp_join_room", {
        joinCode: joinCode.trim(),
        playerName: joinPlayerName.trim(),
      });
      setJoinStatus("running");
    } catch (e) {
      setJoinStatus("error");
      setJoinError(typeof e === "string" ? e : (e as Error).message ?? "加入失败");
    }
  }, [joinCode, joinPlayerName]);

  const handleJoinDisconnect = useCallback(async () => {
    try { await invoke<void>("mp_disconnect"); } catch { /* ignore */ }
    setJoinStatus("idle");
    setJoinError(null);
  }, []);

  // ── 工具 ──
  const installOpenp2p = useCallback(async (srcPath: string): Promise<string> => {
    return await invoke<string>("mp_install_openp2p", { srcPath });
  }, []);

  return (
    <MultiplayerContext.Provider
      value={{
        mode, setMode,
        hostRoomName: hostRoomNameRef.current,
        hostPort, setHostPort,
        hostJoinCode,
        hostStatus, hostError,
        handleHostRoom, handleHostDisconnect,
        joinCode, setJoinCode,
        joinPlayerName, setJoinPlayerName,
        joinStatus, joinError,
        handleJoinRoom, handleJoinDisconnect,
        installOpenp2p,
      }}
    >
      {children}
    </MultiplayerContext.Provider>
  );
}
