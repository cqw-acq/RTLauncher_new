import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { tauriMultiplayerApi } from "@/components/multiplayer/multiplayer-api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invokeMock = vi.mocked(invoke);

describe("tauriMultiplayerApi", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("uses the existing host command contract", async () => {
    invokeMock.mockResolvedValueOnce("room-code").mockResolvedValueOnce("/openp2p");

    const roomInfo = await tauriMultiplayerApi.encodeRoomInfo("room", "25565");
    const path = await tauriMultiplayerApi.startHost("room");

    expect(roomInfo).toBe("room-code");
    expect(path).toBe("/openp2p");
    expect(invokeMock).toHaveBeenNthCalledWith(1, "mp_encode_room_info", {
      roomName: "room",
      portCount: "25565",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "mp_start_openp2p_host", {
      roomName: "room",
    });
  });

  it("uses the existing join command contract", async () => {
    invokeMock.mockResolvedValue("/openp2p");

    await tauriMultiplayerApi.startJoin("room-code", "player");

    expect(invokeMock).toHaveBeenCalledWith("mp_start_openp2p_join", {
      encodedValue: "room-code",
      playerName: "player",
    });
  });

  it("does not check a process when OpenP2P is not installed", async () => {
    invokeMock.mockResolvedValueOnce(false);

    await expect(tauriMultiplayerApi.readStatus()).resolves.toEqual({
      installed: false,
      running: false,
    });
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("mp_check_openp2p");
  });
});
