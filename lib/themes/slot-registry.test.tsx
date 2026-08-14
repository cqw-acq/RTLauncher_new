import type { ComponentType } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ThemeSlotComponentProps } from "./protocol";
import { ThemeSlotRegistry } from "./slot-registry";

const First: ComponentType<ThemeSlotComponentProps> = () => null;
const Second: ComponentType<ThemeSlotComponentProps> = () => null;
const Replacement: ComponentType<ThemeSlotComponentProps> = () => null;
const Wrapper: ComponentType<ThemeSlotComponentProps> = () => null;

describe("ThemeSlotRegistry", () => {
  it("resolves each slot mode for the active owner", () => {
    const registry = new ThemeSlotRegistry();
    const slots = registry.forOwner("com.example.nebula");
    slots.register({ id: "nebula.before", target: "app.sidebar", mode: "before", component: First });
    slots.register({ id: "nebula.after", target: "app.sidebar", mode: "after", component: Second });
    slots.register({ id: "nebula.replace", target: "app.sidebar", mode: "replace", component: Replacement });
    slots.register({ id: "nebula.wrap", target: "app.sidebar", mode: "wrap", component: Wrapper });

    expect(registry.resolve("app.sidebar", "com.example.nebula")).toEqual({
      replacement: expect.objectContaining({ id: "nebula.replace", component: Replacement }),
      before: [expect.objectContaining({ id: "nebula.before", component: First })],
      after: [expect.objectContaining({ id: "nebula.after", component: Second })],
      wrappers: [expect.objectContaining({ id: "nebula.wrap", component: Wrapper })],
    });
    expect(registry.resolve("app.sidebar", "another.theme")).toEqual({
      replacement: undefined,
      before: [],
      after: [],
      wrappers: [],
    });
  });

  it("orders contributions by order and then registration", () => {
    const registry = new ThemeSlotRegistry();
    const slots = registry.forOwner("com.example.nebula");
    slots.register({ id: "nebula.second", target: "app.sidebar.footer", mode: "before", order: 20, component: Second });
    slots.register({ id: "nebula.first", target: "app.sidebar.footer", mode: "before", order: 10, component: First });
    slots.register({ id: "nebula.same-order", target: "app.sidebar.footer", mode: "before", order: 20, component: Wrapper });

    expect(
      registry.resolve("app.sidebar.footer", "com.example.nebula").before.map((item) => item.id),
    ).toEqual(["nebula.first", "nebula.second", "nebula.same-order"]);
  });

  it("rejects duplicate IDs and replacement conflicts", () => {
    const registry = new ThemeSlotRegistry();
    const slots = registry.forOwner("com.example.nebula");
    slots.register({ id: "nebula.sidebar", target: "app.sidebar", mode: "replace", component: Replacement });

    expect(() =>
      slots.register({ id: "nebula.sidebar", target: "app.sidebar.footer", mode: "after", component: First }),
    ).toThrow(/already registered/i);
    expect(() =>
      slots.register({ id: "nebula.sidebar-two", target: "app.sidebar", mode: "replace", component: Second }),
    ).toThrow(/replacement/i);
  });

  it("removes registrations by disposer or owner", () => {
    const registry = new ThemeSlotRegistry();
    const remove = registry.forOwner("com.example.nebula").register({
      id: "nebula.sidebar",
      target: "app.sidebar",
      mode: "after",
      component: First,
    });
    registry.forOwner("com.example.solar").register({
      id: "solar.sidebar",
      target: "app.sidebar",
      mode: "after",
      component: Second,
    });

    remove();
    remove();
    registry.clearOwner("com.example.nebula");

    expect(registry.resolve("app.sidebar", "com.example.nebula").after).toEqual([]);
    expect(registry.resolve("app.sidebar", "com.example.solar").after).toEqual([
      expect.objectContaining({ id: "solar.sidebar" }),
    ]);
  });

  it("notifies subscribers once for each state change", () => {
    const registry = new ThemeSlotRegistry();
    const listener = vi.fn();
    const unsubscribe = registry.subscribe(listener);
    const first = registry.getSnapshot();
    const remove = registry.forOwner("com.example.nebula").register({
      id: "nebula.sidebar",
      target: "app.sidebar",
      mode: "after",
      component: First,
    });

    expect(registry.getSnapshot().revision).toBe(first.revision + 1);
    remove();
    unsubscribe();
    registry.forOwner("com.example.solar").register({
      id: "solar.sidebar",
      target: "app.sidebar",
      mode: "after",
      component: Second,
    });

    expect(listener).toHaveBeenCalledTimes(2);
  });
});
