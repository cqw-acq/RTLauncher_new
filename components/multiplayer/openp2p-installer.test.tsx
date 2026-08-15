import "@testing-library/jest-dom/vitest";

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OpenP2PInstaller } from "@/components/multiplayer/openp2p-installer";

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

vi.mock("@/components/multiplayer/multiplayer-provider", () => ({
  useMultiplayerContext: () => ({
    status: "not_installed",
    installOpenP2P: vi.fn(),
  }),
}));

afterEach(cleanup);

describe("OpenP2PInstaller", () => {
  it("shows the drag-and-drop installer when OpenP2P is missing", async () => {
    render(<OpenP2PInstaller />);

    expect(await screen.findByText("安装 OpenP2P")).toBeInTheDocument();
    expect(
      screen.getByText("将 openp2p 可执行文件拖到此处")
    ).toBeInTheDocument();
  });
});
