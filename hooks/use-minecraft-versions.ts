import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type {
  ClassifiedVersions,
  MinecraftVersion,
  MinecraftVersionType,
} from "@/types";

/**
 * 从后端获取 Minecraft 版本列表的 Hook
 * 调用 Tauri 命令 classify_minecraft_versions 获取真实数据
 */
export function useMinecraftVersions() {
  const [versions, setVersions] = useState<MinecraftVersion[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchVersions = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<ClassifiedVersions>(
        "classify_minecraft_versions"
      );

      const [releases, snapshots, aprilFools, oldVersions] = data;

      const mapVersions = (
        items: { id: string; releaseTime: string }[],
        type: MinecraftVersionType
      ): MinecraftVersion[] =>
        items.map((v) => ({
          id: v.id,
          type,
          releaseDate: v.releaseTime.split("T")[0],
        }));

      const allVersions: MinecraftVersion[] = [
        ...mapVersions(releases, "release"),
        ...mapVersions(snapshots, "snapshot"),
        ...mapVersions(aprilFools, "april_fools"),
        ...mapVersions(oldVersions, "old_version"),
      ];

      // 按发布日期降序排列
      allVersions.sort(
        (a, b) =>
          new Date(b.releaseDate).getTime() -
          new Date(a.releaseDate).getTime()
      );

      // 标记最新正式版
      const latestRelease = allVersions.find((v) => v.type === "release");
      if (latestRelease) {
        latestRelease.isLatest = true;
      }

      setVersions(allVersions);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchVersions();
  }, [fetchVersions]);

  return { versions, loading, error, refetch: fetchVersions };
}
