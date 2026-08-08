"use client";

import { useState, useEffect, useCallback, useMemo, useRef } from "react";
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
  const requestIdRef = useRef(0);

  // 调用方经常以内联数组传入筛选器。用内容生成稳定的数组，避免数组
  // 引用变化导致读取回调和 effect 在每次渲染后重复执行。
  const extensionsKey = extensionsFilter?.join(",") ?? "";
  const stableExtensionsFilter = useMemo(
    () => (extensionsKey ? extensionsKey.split(",") : []),
    [extensionsKey],
  );

  const fetch = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    if (!dirPath) {
      setEntries([]);
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<DirEntry[]>("vm_list_dir", {
        dirPath,
        extensionsFilter: stableExtensionsFilter,
      });
      if (requestId === requestIdRef.current) {
        setEntries(data);
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
  }, [dirPath, stableExtensionsFilter]);

  useEffect(() => {
    void fetch();
    return () => {
      requestIdRef.current += 1;
    };
  }, [fetch]);

  return { entries, loading, error, refetch: fetch };
}
