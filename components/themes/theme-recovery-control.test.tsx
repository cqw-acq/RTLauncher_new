import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { BUILTIN_THEME_ID } from "@/lib/themes/protocol";
import { ThemeRecoveryControl } from "./theme-recovery-control";

const activateTheme = vi.fn(async () => true);

vi.mock("@/components/i18n/use-i18n", () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));
vi.mock("./theme-runtime-provider", () => ({
  useThemeRuntime: () => ({
    snapshot: { activeThemeId: "com.example.nebula" },
    activateTheme,
  }),
}));

afterEach(() => {
  cleanup();
  activateTheme.mockClear();
});

describe("ThemeRecoveryControl", () => {
  it("restores the built-in Theme from outside replaceable content", async () => {
    render(<ThemeRecoveryControl />);

    fireEvent.click(screen.getByRole("button", {
      name: "settings.themeManager.recoverBuiltIn",
    }));

    await waitFor(() => expect(activateTheme).toHaveBeenCalledWith(BUILTIN_THEME_ID));
  });
});
