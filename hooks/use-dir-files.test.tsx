import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useDirFiles } from "./use-dir-files";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  delete (window as typeof window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

describe("useDirFiles", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  });

  it("does not reload when an inline extension filter keeps the same contents", async () => {
    invokeMock.mockResolvedValue([
      { name: "image.png", is_dir: false, extension: "png", size: 42 },
    ]);

    renderHook(() => useDirFiles("/minecraft/screenshots", ["png"]));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
    });

    await new Promise((resolve) => window.setTimeout(resolve, 30));

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
