"use client";

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LevelDatInfo } from "@/types";

interface UseWorldInfoReturn {
  info: LevelDatInfo | null;
  loading: boolean;
  error: string | null;
  refetch: () => void;
  /** 修改游戏规则。支持: keepInventory / mobGriefing / doFireTick / allowCommands */
  modifyGameRule: (
    paramName: "keepInventory" | "mobGriefing" | "doFireTick" | "allowCommands",
    newValue: boolean
  ) => Promise<void>;
}

/**
 * 解析世界目录下的 level.dat，返回种子和游戏规则，并提供修改游戏规则的方法。
 *
 * @param worldFolderPath  包含 `level.dat` 的世界文件夹绝对路径。
 *                         为空字符串或 undefined 时不发起请求。
 */
export function useWorldInfo(worldFolderPath?: string): UseWorldInfoReturn {
  const [info, setInfo] = useState<LevelDatInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    if (!worldFolderPath) {
      setInfo(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<LevelDatInfo>("vm_parse_level_dat", {
        worldFolderPath,
      });
      setInfo(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [worldFolderPath]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  const modifyGameRule = useCallback(
    async (
      paramName: "keepInventory" | "mobGriefing" | "doFireTick" | "allowCommands",
      newValue: boolean
    ) => {
      if (!worldFolderPath) throw new Error("未指定世界文件夹路径");
      await invoke("vm_modify_game_rule", {
        worldFolderPath,
        paramName,
        newValue: newValue.toString(),
      });
      // 修改成功后刷新数据
      await fetch();
    },
    [worldFolderPath, fetch]
  );

  return { info, loading, error, refetch: fetch, modifyGameRule };
}
