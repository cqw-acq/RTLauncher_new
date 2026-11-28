import { describe, expect, it } from "vitest";
import { detectLoaderKeyword, resolveDependencyTarget } from "./mod-dependency-target";

describe("detectLoaderKeyword", () => {
  it("prefers neoforge over forge", () => {
    expect(detectLoaderKeyword("NeoForge")).toBe("neoforge");
    expect(detectLoaderKeyword("1.21.1-neoforge-21.1.267")).toBe("neoforge");
    expect(detectLoaderKeyword("1.20.1-forge-47.4.29")).toBe("forge");
  });

  it("finds loaders inside custom folder names", () => {
    expect(detectLoaderKeyword("e2e-fabric-1.21.1")).toBe("fabric");
    expect(detectLoaderKeyword("quilt-loader-0.30.2-1.20.1")).toBe("quilt");
    expect(detectLoaderKeyword("Fabulously Optimized-1.21.1")).toBeUndefined();
    expect(detectLoaderKeyword("Vanilla")).toBeUndefined();
  });
});

describe("resolveDependencyTarget", () => {
  it("uses the scanned instance before the launch config", () => {
    expect(
      resolveDependencyTarget(
        { minecraft_version: "1.21.1", loader: "Fabric" },
        { minecraftVersion: "1.20.1", versionName: "1.20.1", loadName: "1.20.1-forge-47.4.29" },
      ),
    ).toEqual({ mcVersion: "1.21.1", loader: "fabric" });
  });

  it("falls back to the launch config when the instance has no loader", () => {
    expect(
      resolveDependencyTarget(
        { minecraft_version: "1.21.1", loader: "Vanilla" },
        { loadName: "e2e-fabric-1.21.1" },
      ),
    ).toEqual({ mcVersion: "1.21.1", loader: "fabric" });
    expect(
      resolveDependencyTarget(null, { minecraftVersion: "1.20.1", loadName: "1.20.1-neoforge-47.1.3" }),
    ).toEqual({ mcVersion: "1.20.1", loader: "neoforge" });
  });

  it("keeps the previous defaults when nothing is known", () => {
    expect(resolveDependencyTarget(null, {})).toEqual({ mcVersion: "1.20.1", loader: "forge" });
  });
});
