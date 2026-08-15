import { cleanup, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useMultiplayerLogPolling } from "@/components/multiplayer/use-multiplayer-log-polling";

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("useMultiplayerLogPolling", () => {
  it("polls immediately and every second while active", async () => {
    vi.useFakeTimers();
    const pollLog = vi.fn().mockResolvedValue("");

    renderHook(() => useMultiplayerLogPolling(true, pollLog));
    expect(pollLog).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(2_000);
    expect(pollLog).toHaveBeenCalledTimes(3);
  });

  it("stops polling after unmount", async () => {
    vi.useFakeTimers();
    const pollLog = vi.fn().mockResolvedValue("");
    const { unmount } = renderHook(() =>
      useMultiplayerLogPolling(true, pollLog)
    );

    unmount();
    await vi.advanceTimersByTimeAsync(2_000);

    expect(pollLog).toHaveBeenCalledTimes(1);
  });
});
