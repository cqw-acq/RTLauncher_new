"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { InstanceData } from "@/types";

interface UseInstancesReturn {
  instances: InstanceData[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
}

// 首页卸载与实例详情页挂载之间可能重叠。仅复用进行中的扫描：这样可以
// 消除首次跳转的并发读取，同时不会让之后的版本安装结果被长期缓存。
const inFlightInstanceScans = new Map<string, Promise<InstanceData[]>>();

function loadInstances(instancesPath: string): Promise<InstanceData[]> {
  const pending = inFlightInstanceScans.get(instancesPath);
  if (pending) return pending;

  const request = invoke<InstanceData[]>("vm_scan_instances", {
    instancesPath,
  });
  inFlightInstanceScans.set(instancesPath, request);

  void request.then(
    () => {
      if (inFlightInstanceScans.get(instancesPath) === request) {
        inFlightInstanceScans.delete(instancesPath);
      }
    },
    () => {
      if (inFlightInstanceScans.get(instancesPath) === request) {
        inFlightInstanceScans.delete(instancesPath);
      }
    }
  );

  return request;
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
  const requestIdRef = useRef(0);

  const fetch = useCallback(async () => {
    const requestId = ++requestIdRef.current;
    if (!instancesPath) {
      setInstances([]);
      setLoading(false);
      setError(null);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const data = await loadInstances(instancesPath);
      if (requestId === requestIdRef.current) {
        setInstances(data);
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
  }, [instancesPath]);

  useEffect(() => {
    void fetch();
    return () => {
      requestIdRef.current += 1;
    };
  }, [fetch]);

  return {
    instances,
    loading,
    error,
    refetch: () => {
      void fetch();
    },
  };
}
