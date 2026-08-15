import "@testing-library/jest-dom/vitest";

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AppUpdateBadge, setAppUpdateState } from "./app-updater";

const { checkMock, pushMock } = vi.hoisted(() => ({
  checkMock: vi.fn(),
  pushMock: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-updater", () => ({ check: checkMock }));
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: pushMock }) }));
vi.mock("@/components/i18n/use-i18n", () => ({
  useI18n: () => ({
    language: "zh-CN",
    t: (key: string, values?: Record<string, string>) => {
      if (key === "settings.appUpdater.newVersionVVersionViewDetails") {
        return `发现新版本 v${values?.version ?? ""} · 点击查看`;
      }
      if (key === "settings.appUpdater.newVersionVVersion") {
        return `发现新版本 v${values?.version ?? ""}`;
      }
      if (key === "settings.appUpdater.installNow") return "立即安装";
      if (key === "settings.appUpdater.later") return "稍后再说";
      return key;
    },
  }),
}));

afterEach(() => {
  cleanup();
  setAppUpdateState({ kind: "idle" });
});

describe("AppUpdateBadge", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("uses the prepared custom update when the user accepts", () => {
    setAppUpdateState({
      kind: "available",
      version: "1.2.0",
      notes: "新增启动更新提醒",
      prepared: true,
    });
    render(<AppUpdateBadge />);

    fireEvent.click(
      screen.getByRole("button", { name: "发现新版本 v1.2.0 · 点击查看" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "立即安装" }));

    expect(pushMock).toHaveBeenCalledWith(
      "/check-update?autoStart=1&preparedVersion=1.2.0",
    );
    expect(checkMock).not.toHaveBeenCalled();
  });

  it("keeps plugin updates on the plugin install flow", async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    checkMock.mockResolvedValue({ available: true, downloadAndInstall });
    vi.spyOn(window, "alert").mockImplementation(() => undefined);
    setAppUpdateState({
      kind: "available",
      version: "1.2.0",
      notes: "Plugin release notes",
      prepared: false,
    });
    render(<AppUpdateBadge />);

    fireEvent.click(
      screen.getByRole("button", { name: "发现新版本 v1.2.0 · 点击查看" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "立即安装" }));

    await waitFor(() => expect(downloadAndInstall).toHaveBeenCalledTimes(1));
    expect(pushMock).not.toHaveBeenCalled();
  });
});
