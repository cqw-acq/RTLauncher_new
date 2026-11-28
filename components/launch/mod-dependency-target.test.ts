import { describe, expect, it } from "vitest";
import { dependencyInstanceArgs, detectLoaderKeyword, resolveDependencyTarget } from "./mod-dependency-target";

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

describe("dependencyInstanceArgs", () => {
  it("passes the selected instance to the dependency lookup", () => {
    expect(
      dependencyInstanceArgs(
        { minecraft_version: "1.21.1", loader: "Quilt" },
        "/mc/versions/1.21.1-quilt",
      ),
    ).toEqual({
      instanceDir: "/mc/versions/1.21.1-quilt",
      instanceMcVersion: "1.21.1",
      instanceLoader: "quilt",
    });
  });

  it("falls back to the download cache without a usable selected instance", () => {
    expect(dependencyInstanceArgs(null, "/mc/versions/1.21.1-fabric")).toEqual({});
    expect(dependencyInstanceArgs({ minecraft_version: "1.21.1", loader: "Fabric" }, undefined)).toEqual({});
    expect(dependencyInstanceArgs({ minecraft_version: "1.21.1", loader: "Vanilla" }, "/mc/versions/1.21.1")).toEqual({});
    expect(dependencyInstanceArgs({ minecraft_version: "", loader: "Fabric" }, "/mc/versions/x")).toEqual({});
  });
});
