import { render, screen, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const state = { pathname: "/game-settings/mods/" };
const push = vi.fn();

vi.mock("next/navigation", () => ({
  usePathname: () => state.pathname,
  useRouter: () => ({ push }),
}));

vi.mock("@/components/i18n/use-i18n", () => ({
  useI18n: () => ({
    language: "zh-CN",
    isEnglish: false,
    t: (key: string) => key,
  }),
}));

import GameSettingsLayout from "./layout";

afterEach(() => {
  cleanup();
});

describe("GameSettingsLayout", () => {
  it("highlights the active tab when pathname ends with a slash", () => {
    state.pathname = "/game-settings/mods/";
    render(
      <GameSettingsLayout>
        <div />
      </GameSettingsLayout>,
    );
    expect(screen.getByText("panel.mods").className).toContain("bg-accent");
  });

  it("highlights the active tab when pathname has no trailing slash", () => {
    state.pathname = "/game-settings/mods";
    render(
      <GameSettingsLayout>
        <div />
      </GameSettingsLayout>,
    );
    expect(screen.getByText("panel.mods").className).toContain("bg-accent");
  });

  it("highlights the parent tab for a nested route", () => {
    state.pathname = "/game-settings/schematics/viewer";
    render(
      <GameSettingsLayout>
      <div />
      </GameSettingsLayout>,
    );
    expect(screen.getByText("home.cardGrid.schematics").className).toContain("bg-accent");
  });
});