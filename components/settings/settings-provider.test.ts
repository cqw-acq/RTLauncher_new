import { describe, expect, it } from "vitest";

import {
  isThemeStyle,
  languageFromSystemPreference,
  normalizeThemeStyle,
} from "./settings-provider";

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

describe("isThemeStyle", () => {
  it("accepts all supported color schemes", () => {
    expect(isThemeStyle("classic")).toBe(true);
    expect(isThemeStyle("redstone-terminal")).toBe(true);
    expect(isThemeStyle("grid-command")).toBe(true);
  });

  it("rejects unsupported saved values", () => {
    expect(isThemeStyle("square")).toBe(false);
    expect(isThemeStyle("neon")).toBe(false);
    expect(isThemeStyle(undefined)).toBe(false);
  });
});

describe("normalizeThemeStyle", () => {
  it("keeps current values and migrates old draft values", () => {
    expect(normalizeThemeStyle("classic")).toBe("classic");
    expect(normalizeThemeStyle("redstone-terminal")).toBe("redstone-terminal");
    expect(normalizeThemeStyle("grid-command")).toBe("grid-command");
    expect(normalizeThemeStyle("square")).toBe("grid-command");
    expect(normalizeThemeStyle("glass")).toBe("redstone-terminal");
  });

  it("falls back to the previous default style for missing values", () => {
    expect(normalizeThemeStyle("neon")).toBe("classic");
    expect(normalizeThemeStyle(undefined)).toBe("classic");
  });
});
