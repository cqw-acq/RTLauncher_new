import { afterEach, describe, expect, it, vi } from "vitest";

import { DevelopmentThemeWatcher } from "./development-watcher";

afterEach(() => vi.useRealTimers());

describe("DevelopmentThemeWatcher", () => {
  it("coalesces repeated changes into one reload", async () => {
    vi.useFakeTimers();
    const reload = vi.fn(async () => undefined);
    const watcher = new DevelopmentThemeWatcher({ debounceMs: 200, reload });

    watcher.notifyChange("com.example.nebula");
    vi.advanceTimersByTime(100);
    watcher.notifyChange("com.example.nebula");
    watcher.notifyChange("com.example.nebula");
    await vi.advanceTimersByTimeAsync(200);

    expect(reload).toHaveBeenCalledOnce();
    expect(reload).toHaveBeenCalledWith("com.example.nebula");
  });

  it("cancels pending reloads when disposed", async () => {
    vi.useFakeTimers();
    const reload = vi.fn(async () => undefined);
    const watcher = new DevelopmentThemeWatcher({ reload });
    watcher.notifyChange("com.example.nebula");

    watcher.dispose();
    await vi.runAllTimersAsync();

    expect(reload).not.toHaveBeenCalled();
  });
});
