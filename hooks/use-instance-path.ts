"use client";

import { useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { useInstances } from "@/hooks/use-instances";
import type { InstanceData } from "@/types";

const ensuredInstanceDirs = new Set<string>();
const pendingInstanceDirEnsures = new Map<string, Promise<void>>();

function ensureInstanceDirs(instanceDir: string): Promise<void> {
  if (ensuredInstanceDirs.has(instanceDir)) {
    return Promise.resolve();
  }

  const pending = pendingInstanceDirEnsures.get(instanceDir);
  if (pending) return pending;

  const request = invoke("vm_ensure_instance_dirs", { instanceDir })
    .then(() => {
      ensuredInstanceDirs.add(instanceDir);
    })
    .finally(() => {
      pendingInstanceDirEnsures.delete(instanceDir);
    });

  pendingInstanceDirEnsures.set(instanceDir, request);
  return request;
}

export interface InstancePathInfo {
  /** <minecraftPath>/versions/<name>，无实例时为 undefined */
  instanceDir: string | undefined;
  /** 当前选中的实例（可能为 null，此时需要用 fallback 逻辑） */
  selectedInstance: InstanceData | null;
  /** 实例文件夹名（用于拼接子目录） */
  instanceFolderName: string | undefined;
  /** .minecraft 根目录 */
  minecraftPath: string;
  loading: boolean;
  error: string | null;
  /** 配置是否已加载完成 */
  configLoaded: boolean;
}

/** 从版本文件夹名称中提取原始 Minecraft 版本号 */
function extractMcVersion(name: string): string {
  // 匹配 "x.y.z" 或 "x.y" 格式
  const match = name.match(/\d+\.\d+(?:\.\d+)?/);
  return match ? match[0] : name;
}

/** 从文件夹名中推断加载器类型（返回前端可显示的字符串） */
function inferLoaderFromFolderName(name: string): string {
  const lower = name.toLowerCase();
  if (lower.includes("optifine")) return "OptiFine";
  if (lower.includes("neoforge") || lower.includes("neoforged")) return "NeoForge";
  if (lower.includes("fabric")) return "Fabric";
  if (lower.includes("quilt")) return "Quilt";
  if (lower.includes("forge")) return "Forge";
  if (lower.includes("liteloader")) return "LiteLoader";
  return "Vanilla";
}

/** 从 LaunchContext 和 useInstances 合并出当前实例目录信息 */
export function useInstancePath(): InstancePathInfo {
  const { config, configLoaded } = useLaunchContext();

  const instancesPath = config.minecraftPath
    ? `${config.minecraftPath}/versions`
    : undefined;

  const { instances, loading, error } = useInstances(instancesPath);

  // 关键修复：
  // version-selector-dialog 设置 config.versionName 为纯版本号（如 "1.21.1"），
  // config.loadName 为完整文件夹名（如 "1.21.1-neoforge-4.0.1.20"）。
  // 而 instance.name 是版本文件夹名（如 "1.21.1-neoforge-4.0.1.20"）。
  // 所以优先用 config.loadName 匹配，其次用 config.versionName 匹配。
  const selectedInstance = useMemo(() => {
    if (!instances || instances.length === 0) return null;

    // 方案1：如果 loadName 非空且能找到匹配项
    if (config.loadName) {
      const match = instances.find((i) => i.name === config.loadName);
      if (match) return match;
    }

    // 方案2：用 versionName 精确匹配
    const match = instances.find((i) => i.name === config.versionName);
    if (match) return match;

    // 方案3：用 versionName 前缀匹配（例如 versionName="1.21.1" 匹配 name="1.21.1-neoforge-4.0.1.20"）
    if (config.versionName) {
      const prefixMatch = instances.find((i) =>
        i.name === config.versionName || i.name.startsWith(`${config.versionName}-`)
      );
      if (prefixMatch) return prefixMatch;
    }

    // 方案4：fallback 到第一个实例
    return instances[0];
  }, [instances, config.loadName, config.versionName]);

  // 计算实例文件夹名：优先从 selectedInstance 获取，其次用 config.loadName，最后用 config.versionName
  const instanceFolderName = selectedInstance?.name
    ?? (config.loadName || undefined)
    ?? (config.versionName || undefined);

  // 构建实例目录
  const instanceDir = config.minecraftPath && instanceFolderName
    ? `${config.minecraftPath}/versions/${instanceFolderName}`
    : undefined;

  useEffect(() => {
    // 等待扫描确认实例后再创建标准目录；否则首页与目标页面会在路由
    // 切换期间同时对同一目录发起写入请求。
    if (instanceDir && selectedInstance) {
      ensureInstanceDirs(instanceDir).catch((e) =>
        console.warn("ensure_instance_dirs failed:", e)
      );
    }
  }, [instanceDir, selectedInstance]);

  return {
    instanceDir,
    selectedInstance,
    instanceFolderName,
    minecraftPath: config.minecraftPath,
    loading,
    error,
    configLoaded,
  };
}

/** 从实例或配置中提取 Minecraft 版本号 */
export function getMcVersion(
  selectedInstance: InstanceData | null,
  configVersionName: string | undefined,
): string | undefined {
  // 优先使用 selectedInstance 的 minecraft_version 字段
  if (selectedInstance?.minecraft_version) {
    return selectedInstance.minecraft_version;
  }
  // fallback：从 config.versionName 提取
  if (configVersionName) {
    return extractMcVersion(configVersionName);
  }
  return undefined;
}

/** 从实例或配置中推断 Mod Loader 类型（前端字符串表示） */
export function getModLoader(
  selectedInstance: InstanceData | null,
  configLoadType: string,
  configLoadName: string | undefined,
  configVersionName: string | undefined,
): string | undefined {
  // 优先从 config.loadName 推断（用户在版本选择对话框中明确选择的，最准确）
  if (configLoadType !== "0" && configLoadName) {
    const inferred = inferLoaderFromFolderName(configLoadName);
    if (inferred !== "Vanilla") {
      return inferred;
    }
  }

  // 其次使用 selectedInstance 的 loader 字段
  if (selectedInstance?.loader && selectedInstance.loader !== "Vanilla") {
    return selectedInstance.loader;
  }

  // 从 config.versionName 推断（最后手段）
  if (configVersionName) {
    const inferred = inferLoaderFromFolderName(configVersionName);
    if (inferred !== "Vanilla") {
      return inferred;
    }
  }

  return undefined;
}