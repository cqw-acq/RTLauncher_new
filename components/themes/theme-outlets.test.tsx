import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

import { ThemeRoute } from "./theme-route";
import { ThemeSlot } from "./theme-slot";
import { ThemeRouteRegistry } from "@/lib/themes/route-registry";
import { ThemeSlotRegistry } from "@/lib/themes/slot-registry";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("ThemeSlot", () => {
  it("composes before, replacement, wrapper, and after contributions", () => {
    const registry = new ThemeSlotRegistry();
    const slots = registry.forOwner("com.example.nebula");
    slots.register({
      id: "nebula.before",
      target: "app.sidebar",
      mode: "before",
      component: () => <span>before|</span>,
    });
    slots.register({
      id: "nebula.replace",
      target: "app.sidebar",
      mode: "replace",
      component: () => <span>replacement</span>,
    });
    slots.register({
      id: "nebula.wrap",
      target: "app.sidebar",
      mode: "wrap",
      component: ({ children }) => <span>wrap[{children}]</span>,
    });
    slots.register({
      id: "nebula.after",
      target: "app.sidebar",
      mode: "after",
      component: () => <span>|after</span>,
    });

    render(
      <ThemeSlot registry={registry} owner="com.example.nebula" slotId="app.sidebar">
        built-in
      </ThemeSlot>,
    );

    expect(screen.getByTestId("slot-result").textContent).toBe(
      "before|wrap[replacement]|after",
    );
  });

  it("uses built-in content when there is no contribution", () => {
    render(
      <ThemeSlot
        registry={new ThemeSlotRegistry()}
        owner="com.example.nebula"
        slotId="app.sidebar"
      >
        <span>built-in</span>
      </ThemeSlot>,
    );

    expect(screen.getByText("built-in")).toBeTruthy();
  });

  it("falls back only the failed contribution", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const registry = new ThemeSlotRegistry();
    registry.forOwner("com.example.nebula").register({
      id: "nebula.broken",
      target: "app.sidebar",
      mode: "replace",
      component: () => {
        throw new Error("render failed");
      },
    });
    const errors: string[] = [];

    render(
      <ThemeSlot
        registry={registry}
        owner="com.example.nebula"
        slotId="app.sidebar"
        onError={(_error, contributionId) => errors.push(contributionId)}
      >
        <span>built-in</span>
      </ThemeSlot>,
    );

    expect(screen.getByText("built-in")).toBeTruthy();
    expect(errors).toEqual(["nebula.broken"]);
  });
});

describe("ThemeRoute", () => {
  it("wraps a replacement for a core route", () => {
    const registry = new ThemeRouteRegistry();
    const routes = registry.forOwner("com.example.nebula");
    routes.override({
      id: "nebula.home",
      target: "core.home",
      mode: "replace",
      component: () => <span>home</span>,
    });
    routes.override({
      id: "nebula.frame",
      target: "core.home",
      mode: "wrap",
      component: ({ children }) => <span>frame[{children}]</span>,
    });

    render(
      <ThemeRoute
        registry={registry}
        owner="com.example.nebula"
        routeId="core.home"
      >
        built-in
      </ThemeRoute>,
    );

    expect(screen.getByTestId("route-result").textContent).toBe("frame[home]");
  });

  it("renders a virtual Theme page", () => {
    const registry = new ThemeRouteRegistry();
    registry.forOwner("com.example.nebula").add({
      id: "nebula.statistics",
      path: "/theme/nebula/statistics",
      component: ({ routeId }) => <span>virtual:{routeId}</span>,
    });

    render(
      <ThemeRoute
        registry={registry}
        owner="com.example.nebula"
        pathname="/theme/nebula/statistics"
      >
        missing
      </ThemeRoute>,
    );

    expect(screen.getByText("virtual:nebula.statistics")).toBeTruthy();
  });

  it("keeps the built-in route when a wrapper fails", () => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    const registry = new ThemeRouteRegistry();
    registry.forOwner("com.example.nebula").override({
      id: "nebula.broken-frame",
      target: "core.home",
      mode: "wrap",
      component: () => {
        throw new Error("wrapper failed");
      },
    });

    render(
      <ThemeRoute registry={registry} owner="com.example.nebula" routeId="core.home">
        <span>built-in</span>
      </ThemeRoute>,
    );

    expect(screen.getByText("built-in")).toBeTruthy();
  });
});
