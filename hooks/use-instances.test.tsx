import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useInstances } from "./use-instances";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("useInstances", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reuses an in-flight scan when navigation remounts the hook", async () => {
    let resolveInstances: (value: { name: string; minecraft_version: string; loader: string; mods_count: number }[]) => void;
    const pendingInstances = new Promise<{ name: string; minecraft_version: string; loader: string; mods_count: number }[]>((resolve) => {
      resolveInstances = resolve;
    });
    invokeMock.mockReturnValueOnce(pendingInstances as never);

    const firstMount = renderHook(() => useInstances("/minecraft/versions/navigation-test"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
    });

    firstMount.unmount();
    const secondMount = renderHook(() => useInstances("/minecraft/versions/navigation-test"));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
    });

    resolveInstances!([
      {
        name: "example",
        minecraft_version: "1.21.1",
        loader: "Vanilla",
        mods_count: 0,
      },
    ]);

    await waitFor(() => {
      expect(secondMount.result.current.instances).toHaveLength(1);
      expect(secondMount.result.current.loading).toBe(false);
    });
  });
});
