"use client";

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InstanceData } from "@/types";

interface UseInstancesReturn {
  instances: InstanceData[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

/**
 * 扫描 instances 目录，返回所有实例的结构化信息。
 *
 * @param instancesPath  instances 目录的绝对路径（如 `<minecraftPath>/instance`）。
 *                       为空字符串或 undefined 时不发起请求。
 */
export function useInstances(instancesPath?: string): UseInstancesReturn {
  const [instances, setInstances] = useState<InstanceData[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    if (!instancesPath) {
      setInstances([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<InstanceData[]>("vm_scan_instances", {
        instancesPath,
      });
      setInstances(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [instancesPath]);

  useEffect(() => {
    fetch();
  }, [fetch]);

  return { instances, loading, error, refetch: fetch };
}
