import { describe, expect, it } from "vitest";

import { languageFromSystemPreference } from "./settings-provider";

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
