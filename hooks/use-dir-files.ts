"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DirEntry } from "@/types";

interface UseDirFilesReturn {
  entries: DirEntry[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * 列出指定目录的直接子条目（一层，不递归）。
 *
 * @param dirPath          目录绝对路径，为空时不请求
 * @param extensionsFilter 允许通过的文件扩展名列表（小写，不含点），空数组代表不过滤
 */
export function useDirFiles(
  dirPath?: string,
  extensionsFilter?: string[]
): UseDirFilesReturn {
  const [entries, setEntries] = useState<DirEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 稳定化 extensionsFilter 引用，避免每次渲染都重新触发 fetch
  const stableFilter = useMemo(
    () => extensionsFilter ?? [],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [extensionsFilter?.join(",")]
  );

  const fetch = useCallback(async () => {
    if (!dirPath) {
      setEntries([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<DirEntry[]>("vm_list_dir", {
        dirPath,
        extensionsFilter: stableFilter,
      });
      setEntries(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [dirPath, stableFilter]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { entries, loading, error, refetch: fetch };
}
