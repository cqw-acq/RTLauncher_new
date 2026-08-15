import "@testing-library/jest-dom/vitest";

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MultiplayerRunningPanel } from "@/components/multiplayer/multiplayer-running-panel";

afterEach(cleanup);

describe("MultiplayerRunningPanel", () => {
  it("shows host session data and stops the current session", () => {
    const stop = vi.fn();
    render(
      <MultiplayerRunningPanel
        runMode="host"
        roomInfo="cm9vbSwyNTU2NQ=="
        logText={"connected\n"}
        openP2PDirectory="/bridge"
        onStop={stop}
        onClearLog={vi.fn()}
      />
    );

    expect(screen.getByText("房间已创建")).toBeInTheDocument();
    expect(screen.getByText("cm9vbSwyNTU2NQ==")).toBeInTheDocument();
    expect(screen.getByText("connected")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "停止联机" }));
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it("describes an unknown existing process as a background process", () => {
    render(
      <MultiplayerRunningPanel
        runMode={null}
        roomInfo={null}
        logText=""
        openP2PDirectory=""
        onStop={vi.fn()}
        onClearLog={vi.fn()}
      />
    );

    expect(screen.getByText("OpenP2P 已在后台运行")).toBeInTheDocument();
    expect(screen.getByText(/后台进程/)).toBeInTheDocument();
  });
});
