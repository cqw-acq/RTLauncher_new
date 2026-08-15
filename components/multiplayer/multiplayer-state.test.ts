import { describe, expect, it } from "vitest";

import {
  initialMultiplayerState,
  multiplayerReducer,
} from "@/components/multiplayer/multiplayer-state";

describe("multiplayerReducer", () => {
  it("keeps a known run mode when a refresh confirms the process is running", () => {
    const runningState = {
      ...initialMultiplayerState,
      status: "running" as const,
      runMode: "host" as const,
      roomInfo: "b2xkLXJvb20sMjU1NjU=",
    };

    expect(
      multiplayerReducer(runningState, {
        type: "statusResolved",
        installed: true,
        running: true,
      })
    ).toMatchObject({
      status: "running",
      runMode: "host",
      roomInfo: "b2xkLXJvb20sMjU1NjU=",
      errorMsg: null,
    });
  });

  it("clears session data after the process stops", () => {
    const runningState = {
      ...initialMultiplayerState,
      status: "running" as const,
      runMode: "join" as const,
      roomInfo: "b2xkLXJvb20sMjU1NjU=",
      errorMsg: "old error",
    };

    expect(multiplayerReducer(runningState, { type: "stopped" })).toMatchObject({
      status: "installed",
      runMode: null,
      roomInfo: null,
      errorMsg: null,
    });
  });

  it("appends log chunks without joining adjacent lines", () => {
    const withLog = multiplayerReducer(
      { ...initialMultiplayerState, logText: "first" },
      { type: "logReceived", text: "second\n" }
    );

    expect(withLog.logText).toBe("first\nsecond\n");
  });

  it("records a normalized operation error", () => {
    expect(
      multiplayerReducer(initialMultiplayerState, {
        type: "failed",
        message: "启动失败",
      })
    ).toMatchObject({ status: "error", errorMsg: "启动失败" });
  });
});
