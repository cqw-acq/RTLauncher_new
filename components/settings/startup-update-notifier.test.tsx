import "@testing-library/jest-dom/vitest";

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { StartupUpdateNotifier } from "./startup-update-notifier";

const { pushMock } = vi.hoisted(() => ({ pushMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: pushMock }) }));
vi.mock("@/components/i18n/use-i18n", () => ({
  useI18n: () => ({
    t: (key: string, values?: Record<string, string>) => {
      if (key === "settings.appUpdater.newVersionVVersion") {
        return `发现新版本 v${values?.version ?? ""}`;
      }
      if (key === "settings.appUpdater.later") return "稍后再说";
      if (key === "settings.appUpdater.updateNow") return "立即更新";
      if (key === "settings.appUpdater.noReleaseNotesAvailable") return "暂无更新说明";
      return key;
    },
  }),
}));

const invokeMock = vi.mocked(invoke);

afterEach(cleanup);

describe("StartupUpdateNotifier", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the release changelog when startup finds an update", async () => {
    invokeMock.mockResolvedValue({
      needs_check: true,
      update_available: true,
      current_version: "1.1.0",
      target_version: "1.2.0",
      message: "发现新版本可用",
      changelog: "新增启动更新提醒",
    });

    render(<StartupUpdateNotifier delayMs={0} />);

    expect(await screen.findByText("发现新版本 v1.2.0")).toBeInTheDocument();
    expect(screen.getByText("新增启动更新提醒")).toBeInTheDocument();
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("check_for_updates", { force: true });
    });
  });

  it("starts the automatic update flow when the user accepts", async () => {
    invokeMock.mockResolvedValue({
      needs_check: true,
      update_available: true,
      current_version: "1.1.0",
      target_version: "1.2.0",
      message: "发现新版本可用",
      changelog: "新增启动更新提醒",
    });
    render(<StartupUpdateNotifier delayMs={0} />);

    fireEvent.click(await screen.findByRole("button", { name: "立即更新" }));

    expect(pushMock).toHaveBeenCalledWith(
      "/check-update?autoStart=1&preparedVersion=1.2.0",
    );
  });

  it("dismisses the changelog without starting an update", async () => {
    invokeMock.mockResolvedValue({
      needs_check: true,
      update_available: true,
      current_version: "1.1.0",
      target_version: "1.2.0",
      message: "发现新版本可用",
      changelog: "新增启动更新提醒",
    });
    render(<StartupUpdateNotifier delayMs={0} />);

    fireEvent.click(await screen.findByRole("button", { name: "稍后再说" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(pushMock).not.toHaveBeenCalled();
  });

  it("stays hidden when no update is available", async () => {
    invokeMock.mockResolvedValue({
      needs_check: true,
      update_available: false,
      current_version: "1.2.0",
      target_version: null,
      message: "当前已是最新版本",
      changelog: null,
    });

    render(<StartupUpdateNotifier delayMs={0} />);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("stays hidden when the startup check fails", async () => {
    invokeMock.mockRejectedValue(new Error("network unavailable"));

    render(<StartupUpdateNotifier delayMs={0} />);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
