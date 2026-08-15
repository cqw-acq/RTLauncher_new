import { act, cleanup, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { MultiplayerApi } from "@/components/multiplayer/multiplayer-api";
import {
  MultiplayerProvider,
  useMultiplayerContext,
} from "@/components/multiplayer/multiplayer-provider";

afterEach(cleanup);

function createApi(overrides: Partial<MultiplayerApi> = {}): MultiplayerApi {
  return {
    readStatus: vi.fn().mockResolvedValue({ installed: true, running: false }),
    install: vi.fn().mockResolvedValue("/bridge/openp2p"),
    encodeRoomInfo: vi.fn().mockResolvedValue("cm9vbSwyNTU2NQ=="),
    startHost: vi.fn().mockResolvedValue("/bridge/openp2p"),
    startJoin: vi.fn().mockResolvedValue("/bridge/openp2p"),
    stop: vi.fn().mockResolvedValue(undefined),
    isRunning: vi.fn().mockResolvedValue(false),
    pollLog: vi.fn().mockResolvedValue(""),
    getPaths: vi.fn().mockResolvedValue({
      directory: "/bridge",
      executable: "/bridge/openp2p",
    }),
    ...overrides,
  };
}

describe("MultiplayerProvider", () => {
  it("generates the room code before it starts a host session", async () => {
    const events: string[] = [];
    const api = createApi({
      encodeRoomInfo: vi.fn(async () => {
        events.push("encode");
        return "cm9vbSwyNTU2NQ==";
      }),
      startHost: vi.fn(async () => {
        events.push("start");
        return "/bridge/openp2p";
      }),
    });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <MultiplayerProvider api={api}>{children}</MultiplayerProvider>
    );
    const { result } = renderHook(() => useMultiplayerContext(), { wrapper });

    await act(async () => {
      await result.current.startAsHost("room", "25565");
    });

    expect(events).toEqual(["encode", "start"]);
    expect(result.current).toMatchObject({
      status: "running",
      runMode: "host",
      roomInfo: "cm9vbSwyNTU2NQ==",
      errorMsg: null,
    });
  });

  it("exposes a useful error and keeps the failed promise", async () => {
    const api = createApi({
      startJoin: vi.fn().mockRejectedValue(new Error("无法启动")),
    });
    const wrapper = ({ children }: { children: ReactNode }) => (
      <MultiplayerProvider api={api}>{children}</MultiplayerProvider>
    );
    const { result } = renderHook(() => useMultiplayerContext(), { wrapper });

    let caughtError: unknown;
    await act(async () => {
      try {
        await result.current.startAsJoin("code", "player");
      } catch (error) {
        caughtError = error;
      }
    });

    expect(caughtError).toEqual(new Error("无法启动"));
    expect(result.current).toMatchObject({
      status: "error",
      errorMsg: "无法启动",
    });
  });

  it("keeps empty path fallbacks when Tauri cannot resolve paths", async () => {
    const api = createApi({
      getPaths: vi.fn().mockRejectedValue(new Error("path unavailable")),
    });
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const wrapper = ({ children }: { children: ReactNode }) => (
      <MultiplayerProvider api={api}>{children}</MultiplayerProvider>
    );
    const { result } = renderHook(() => useMultiplayerContext(), { wrapper });

    await expect(result.current.getOpenP2PPaths()).resolves.toEqual({
      directory: "",
      executable: "",
    });
    expect(consoleError).toHaveBeenCalledWith(
      "获取 openp2p 路径失败:",
      expect.any(Error)
    );
  });
});
