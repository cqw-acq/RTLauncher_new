"use client";

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ResourcePackInfo } from "@/types";

interface UseResourcePacksReturn {
  packs: ResourcePackInfo[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * 扫描指定根目录下的 resourcepacks/ 子文件夹，返回所有材质包信息。
 *
 * @param rootPath  含有 `resourcepacks/` 子目录的根路径（即实例目录）。
 *                  为空字符串或 undefined 时不发起请求。
 */
export function useResourcePacks(rootPath?: string): UseResourcePacksReturn {
  const [packs, setPacks] = useState<ResourcePackInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    if (!rootPath) {
      setPacks([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<ResourcePackInfo[]>(
        "vm_find_resource_packs",
        { rootPath }
      );
      setPacks(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [rootPath]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { packs, loading, error, refetch: fetch };
}
