"use client";

import { useLaunchContext } from "@/components/launch/launch-provider";
import { useInstances } from "@/hooks/use-instances";
import type { InstanceData } from "@/types";

export interface InstancePathInfo {
  /** <minecraftPath>/instance/<name>，无实例时为 undefined */
  instanceDir: string | undefined;
  /** 当前选中的实例（默认第一个） */
  selectedInstance: InstanceData | null;
  /** .minecraft 根目录 */
  minecraftPath: string;
  loading: boolean;
  error: string | null;
}

/** 从 LaunchContext 和 useInstances 合并出当前实例目录信息 */
export function useInstancePath(): InstancePathInfo {
  const { config } = useLaunchContext();
  const instancesPath = config.minecraftPath
    ? `${config.minecraftPath}/instance`
    : undefined;
  const { instances, loading, error } = useInstances(instancesPath);

  const selectedInstance = instances.length > 0 ? instances[0] : null;
  const instanceDir =
    config.minecraftPath && selectedInstance
      ? `${config.minecraftPath}/instance/${selectedInstance.name}`
      : undefined;

  return {
    instanceDir,
    selectedInstance,
    minecraftPath: config.minecraftPath,
    loading,
    error,
  };
}
