import type { ComponentType } from "react";
import { describe, expect, it } from "vitest";

import type { ThemeRouteComponentProps } from "./protocol";
import { ThemeRouteRegistry } from "./route-registry";

const Home: ComponentType<ThemeRouteComponentProps> = () => null;
const HomeFrame: ComponentType<ThemeRouteComponentProps> = () => null;
const Statistics: ComponentType<ThemeRouteComponentProps> = () => null;

describe("ThemeRouteRegistry", () => {
  it("resolves replace and wrap contributions for the active owner", () => {
    const registry = new ThemeRouteRegistry();
    const routes = registry.forOwner("com.example.nebula");
    routes.override({
      id: "nebula.home",
      target: "core.home",
      mode: "replace",
      component: Home,
    });
    routes.override({
      id: "nebula.home-frame",
      target: "core.home",
      mode: "wrap",
      component: HomeFrame,
    });

    expect(registry.resolveCoreRoute("core.home", "com.example.nebula")).toEqual({
      replacement: expect.objectContaining({ id: "nebula.home", component: Home }),
      wrappers: [expect.objectContaining({ id: "nebula.home-frame", component: HomeFrame })],
    });
    expect(registry.resolveCoreRoute("core.home", "another.theme")).toEqual({
      replacement: undefined,
      wrappers: [],
    });
  });

  it("resolves virtual pages only for their owner", () => {
    const registry = new ThemeRouteRegistry();
    registry.forOwner("com.example.nebula").add({
      id: "nebula.statistics",
      path: "/theme/nebula/statistics/",
      component: Statistics,
    });

    expect(
      registry.resolveVirtualRoute("/theme/nebula/statistics", "com.example.nebula"),
    ).toEqual(expect.objectContaining({ id: "nebula.statistics", component: Statistics }));
    expect(registry.resolveVirtualRoute("/theme/nebula/statistics", "another.theme")).toBeUndefined();
  });

  it("rejects duplicate contribution IDs and route conflicts", () => {
    const registry = new ThemeRouteRegistry();
    const routes = registry.forOwner("com.example.nebula");
    routes.override({
      id: "nebula.home",
      target: "core.home",
      mode: "replace",
      component: Home,
    });

    expect(() =>
      routes.add({
        id: "nebula.home",
        path: "/theme/nebula/home",
        component: Statistics,
      }),
    ).toThrow(/already registered/i);
    expect(() =>
      routes.override({
        id: "nebula.home-two",
        target: "core.home",
        mode: "replace",
        component: Statistics,
      }),
    ).toThrow(/replacement/i);
  });

  it("removes one registration through its disposer", () => {
    const registry = new ThemeRouteRegistry();
    const remove = registry.forOwner("com.example.nebula").add({
      id: "nebula.statistics",
      path: "/theme/nebula/statistics",
      component: Statistics,
    });

    remove();

    expect(
      registry.resolveVirtualRoute("/theme/nebula/statistics", "com.example.nebula"),
    ).toBeUndefined();
  });

  it("clears only registrations owned by the selected theme", () => {
    const registry = new ThemeRouteRegistry();
    registry.forOwner("com.example.nebula").add({
      id: "nebula.statistics",
      path: "/theme/nebula/statistics",
      component: Statistics,
    });
    registry.forOwner("com.example.solar").add({
      id: "solar.statistics",
      path: "/theme/solar/statistics",
      component: Statistics,
    });

    registry.clearOwner("com.example.nebula");

    expect(
      registry.resolveVirtualRoute("/theme/nebula/statistics", "com.example.nebula"),
    ).toBeUndefined();
    expect(
      registry.resolveVirtualRoute("/theme/solar/statistics", "com.example.solar"),
    ).toEqual(expect.objectContaining({ id: "solar.statistics" }));
  });

  it("publishes a new snapshot only when registry state changes", () => {
    const registry = new ThemeRouteRegistry();
    const first = registry.getSnapshot();
    const remove = registry.forOwner("com.example.nebula").add({
      id: "nebula.statistics",
      path: "/theme/nebula/statistics",
      component: Statistics,
    });
    const second = registry.getSnapshot();

    expect(second).not.toBe(first);
    expect(second.revision).toBe(first.revision + 1);

    remove();
    remove();

    expect(registry.getSnapshot().revision).toBe(second.revision + 1);
  });
});
