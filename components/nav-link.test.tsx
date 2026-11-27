import { describe, expect, it } from "vitest";
import { normalizeHref } from "./nav-link";

describe("normalizeHref", () => {
  it("keeps external, protocol-relative and hash-only links unchanged", () => {
    expect(normalizeHref("https://example.com/a")).toBe("https://example.com/a");
    expect(normalizeHref("//cdn.example.com/a")).toBe("//cdn.example.com/a");
    expect(normalizeHref("#top")).toBe("#top");
  });

  it("adds a trailing slash to internal paths", () => {
    expect(normalizeHref("/game-settings/mods")).toBe("/game-settings/mods/");
    expect(normalizeHref("/game-settings/mods/")).toBe("/game-settings/mods/");
    expect(normalizeHref("/")).toBe("/");
  });

  it("only normalizes the path part when there is a query or hash", () => {
    expect(normalizeHref("/download?tab=chinese")).toBe("/download/?tab=chinese");
    expect(normalizeHref("/check-update#top")).toBe("/check-update/#top");
    expect(normalizeHref("/download/?tab=x")).toBe("/download/?tab=x");
  });
});