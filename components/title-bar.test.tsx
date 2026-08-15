import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ThemeSlotRegistry } from "@/lib/themes/slot-registry";
import { TitleBar } from "./title-bar";

const slots = new ThemeSlotRegistry();
const owner = "com.example.nebula@1";

vi.mock("@/components/mode-toggle", () => ({ ModeToggle: () => <span>mode</span> }));
vi.mock("@/components/themes/theme-runtime-provider", () => ({
  useThemeRuntime: () => ({
    slots,
    snapshot: { activeOwner: owner },
    reportThemeError: vi.fn(),
  }),
}));

afterEach(() => {
  cleanup();
  slots.clearOwner(owner);
});

describe("TitleBar", () => {
  it("keeps native window controls when a Theme replaces the action slot", () => {
    slots.forOwner(owner).register({
      id: "nebula.actions",
      target: "app.titlebar.actions",
      mode: "replace",
      component: () => <span>Theme actions</span>,
    });

    render(<TitleBar />);

    expect(screen.getByText("Theme actions")).toBeTruthy();
    expect(screen.getByTitle("最小化")).toBeTruthy();
    expect(screen.getByTitle("最大化")).toBeTruthy();
    expect(screen.getByTitle("关闭")).toBeTruthy();
  });
});
