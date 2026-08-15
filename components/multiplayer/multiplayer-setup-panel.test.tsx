import "@testing-library/jest-dom/vitest";

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MultiplayerSetupPanel } from "@/components/multiplayer/multiplayer-setup-panel";

afterEach(cleanup);

describe("MultiplayerSetupPanel", () => {
  it("starts a host with the current room name and port", async () => {
    const startHost = vi.fn().mockResolvedValue(undefined);
    render(
      <MultiplayerSetupPanel
        onStartHost={startHost}
        onStartJoin={vi.fn()}
      />
    );

    const startButton = screen.getByRole("button", { name: "启动联机" });
    expect(startButton).toBeDisabled();

    fireEvent.change(screen.getByLabelText("房间名"), {
      target: { value: "my_room" },
    });
    fireEvent.click(startButton);

    await waitFor(() => {
      expect(startHost).toHaveBeenCalledWith("my_room", "25565");
    });
  });

  it("switches to join mode and submits the room code and player name", async () => {
    const startJoin = vi.fn().mockResolvedValue(undefined);
    render(
      <MultiplayerSetupPanel
        onStartHost={vi.fn()}
        onStartJoin={startJoin}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "加入房间" }));
    fireEvent.change(screen.getByLabelText("房间编码"), {
      target: { value: "room-code" },
    });
    fireEvent.change(screen.getByLabelText("玩家名"), {
      target: { value: "player" },
    });
    fireEvent.click(screen.getByRole("button", { name: "加入联机" }));

    await waitFor(() => {
      expect(startJoin).toHaveBeenCalledWith("room-code", "player");
    });
  });
});
