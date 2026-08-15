import { afterEach, describe, expect, it, vi } from "vitest";

import { ThemeHealthMonitor } from "./health-monitor";

afterEach(() => vi.useRealTimers());

describe("ThemeHealthMonitor", () => {
  it("rolls back after three errors in thirty seconds", () => {
    vi.useFakeTimers();
    const rollback = vi.fn();
    const monitor = new ThemeHealthMonitor({ onRollback: rollback });
    monitor.start("com.example.nebula");

    monitor.reportError("com.example.nebula");
    vi.advanceTimersByTime(10_000);
    monitor.reportError("com.example.nebula");
    vi.advanceTimersByTime(10_000);
    monitor.reportError("com.example.nebula");

    expect(rollback).toHaveBeenCalledOnce();
    expect(rollback).toHaveBeenCalledWith("com.example.nebula");
  });

  it("expires errors outside the time window", () => {
    vi.useFakeTimers();
    const rollback = vi.fn();
    const monitor = new ThemeHealthMonitor({ onRollback: rollback });
    monitor.start("com.example.nebula");
    monitor.reportError("com.example.nebula");
    monitor.reportError("com.example.nebula");

    vi.advanceTimersByTime(30_001);
    monitor.reportError("com.example.nebula");

    expect(rollback).not.toHaveBeenCalled();
  });

  it("confirms health after a quiet startup period", () => {
    vi.useFakeTimers();
    const healthy = vi.fn();
    const monitor = new ThemeHealthMonitor({
      healthyDelayMs: 5_000,
      onHealthy: healthy,
      onRollback() {},
    });

    monitor.start("com.example.nebula");
    vi.advanceTimersByTime(4_999);
    expect(healthy).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);

    expect(healthy).toHaveBeenCalledWith("com.example.nebula");
  });
});
