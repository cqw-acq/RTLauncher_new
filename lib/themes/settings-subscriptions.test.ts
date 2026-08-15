import { describe, expect, it, vi } from "vitest";

import { ThemeSettingsSubscriptions } from "./settings-subscriptions";

describe("ThemeSettingsSubscriptions", () => {
  it("notifies active subscribers and stops after unsubscribe", () => {
    const subscriptions = new ThemeSettingsSubscriptions();
    const listener = vi.fn();
    const unsubscribe = subscriptions.subscribe(listener);

    subscriptions.notify();
    unsubscribe();
    subscriptions.notify();

    expect(listener).toHaveBeenCalledOnce();
  });

  it("continues after a subscriber throws", () => {
    const subscriptions = new ThemeSettingsSubscriptions();
    const laterListener = vi.fn();
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    subscriptions.subscribe(() => {
      throw new Error("listener failed");
    });
    subscriptions.subscribe(laterListener);

    try {
      expect(() => subscriptions.notify()).not.toThrow();
      expect(laterListener).toHaveBeenCalledOnce();
    } finally {
      consoleError.mockRestore();
    }
  });
});
