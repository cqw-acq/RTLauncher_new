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
});
