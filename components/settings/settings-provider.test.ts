import { describe, expect, it } from "vitest";

import { languageFromSystemPreference, migrateSettingsV3 } from "./settings-provider";

describe("languageFromSystemPreference", () => {
  it("uses Chinese for Chinese system locales", () => {
    expect(languageFromSystemPreference("zh-CN")).toBe("zh-CN");
    expect(languageFromSystemPreference("zh-TW")).toBe("zh-CN");
  });

  it("uses English for other system locales", () => {
    expect(languageFromSystemPreference("en-US")).toBe("en-US");
    expect(languageFromSystemPreference("ja-JP")).toBe("en-US");
  });
});

describe("migrateSettingsV3", () => {
  it("adds the built-in frontend Theme without changing existing appearance settings", () => {
    const migrated = migrateSettingsV3({
      appearance: { themeMode: "dark", fontSize: 16 },
    });

    expect(migrated.appearance).toMatchObject({
      themeId: "builtin.default",
      themeMode: "dark",
      fontSize: 16,
    });
  });
});
