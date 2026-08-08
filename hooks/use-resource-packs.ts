"use client";

import { useState, useEffect, useCallback, useRef } from "react";
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
  const requestIdRef = useRef(0);

  const fetch = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    if (!rootPath) {
      setPacks([]);
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<ResourcePackInfo[]>(
        "vm_find_resource_packs",
        { rootPath }
      );
      if (requestId === requestIdRef.current) {
        setPacks(data);
      }
    } catch (e) {
      if (requestId === requestIdRef.current) {
        setError(String(e));
      }
    } finally {
      if (requestId === requestIdRef.current) {
        setLoading(false);
      }
    }
  }, [rootPath]);

  useEffect(() => {
    void fetch();
    return () => {
      requestIdRef.current += 1;
    };
  }, [fetch]);

  return { packs, loading, error, refetch: fetch };
}
