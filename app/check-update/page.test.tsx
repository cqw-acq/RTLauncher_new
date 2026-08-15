import "@testing-library/jest-dom/vitest";

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { StrictMode } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import CheckUpdatePage from "./page";

vi.mock("@tauri-apps/api/app", () => ({ getVersion: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("next/navigation", () => ({ useRouter: () => ({ back: vi.fn() }) }));
vi.mock("@/components/i18n/use-i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

const invokeMock = vi.mocked(invoke);
const getVersionMock = vi.mocked(getVersion);

function callsFor(command: string) {
  return invokeMock.mock.calls.filter(([name]) => name === command);
}

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "", "/");
});

describe("CheckUpdatePage prepared startup flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getVersionMock.mockResolvedValue("1.1.0");
    invokeMock.mockImplementation(async (command) => {
      if (command === "check_for_updates") {
        return {
          needs_check: true,
          update_available: true,
          current_version: "1.1.0",
          target_version: "1.2.0",
          message: "发现新版本可用",
          changelog: "新增启动更新提醒",
        };
      }
      if (command === "download_update") {
        return { success: true, path: "/tmp/update", size: 10 };
      }
      if (command === "get_update_status") {
        if (callsFor("download_update").length === 0) {
          return {
            status: "available",
            target_version: "1.2.0",
            changelog: "新增启动更新提醒",
          };
        }
        return { status: "downloaded", target_version: "1.2.0", download_progress: 100 };
      }
      if (command === "install_update") {
        return { success: true, message: "installed" };
      }
      throw new Error(`Unexpected command: ${command}`);
    });
  });

  it("downloads the prepared update without checking the network again", async () => {
    window.history.replaceState(
      {},
      "",
      "/check-update?autoStart=1&preparedVersion=1.2.0",
    );

    render(<CheckUpdatePage />);

    await waitFor(() => expect(callsFor("download_update")).toHaveLength(1));
    expect(callsFor("check_for_updates")).toHaveLength(0);
    expect(screen.getByText("新增启动更新提醒")).toBeInTheDocument();
  });

  it("starts a prepared download only once in React Strict Mode", async () => {
    window.history.replaceState(
      {},
      "",
      "/check-update?autoStart=1&preparedVersion=1.2.0",
    );

    render(
      <StrictMode>
        <CheckUpdatePage />
      </StrictMode>,
    );

    await waitFor(() => expect(callsFor("download_update")).toHaveLength(1));
    await new Promise((resolve) => window.setTimeout(resolve, 30));
    expect(callsFor("download_update")).toHaveLength(1);
  });

  it("rejects a prepared URL that does not match the backend target", async () => {
    window.history.replaceState(
      {},
      "",
      "/check-update?autoStart=1&preparedVersion=9.9.9",
    );

    render(<CheckUpdatePage />);

    await waitFor(() => expect(callsFor("get_update_status")).toHaveLength(1));
    expect(callsFor("download_update")).toHaveLength(0);
    expect(callsFor("check_for_updates")).toHaveLength(0);
  });

  it("shows the changelog returned by a manual update check", async () => {
    window.history.replaceState({}, "", "/check-update");

    render(<CheckUpdatePage />);

    expect(await screen.findByText("新增启动更新提醒")).toBeInTheDocument();
  });
});
