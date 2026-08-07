"use client";

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface ModpackListEntry {
  name: string;
  format: "modrinth" | "curseforge";
  file_count: number;
  updated_at: number;
  game_version: string;
  loader?: string;
  optifine?: boolean;
  cross_loader?: boolean;
}

export interface ModrinthFileEntry {
  path: string;
  hashes: { sha1: string; sha512: string; sha256?: string };
  /// 客户端/服务端环境要求：required / optional / unsupported
  env: {
    client: "required" | "optional" | "unsupported";
    server: "required" | "optional" | "unsupported";
  };
  downloads: string[];
  fileSize: number;
  /// 显示用：项目名/文件名（不写入规范结构，但便于 UI 展示）
  display_name?: string;
}

export interface CurseforgeFileEntry {
  projectID: number;
  fileID: number;
  display_name?: string;
  required?: boolean;
  category?: string;
}

export type ModpackInstance =
  | {
      /// mrpack 标准字段
      formatVersion: number;
      game: string;
      versionId: string;
      name: string;
      summary?: string;
      files: ModrinthFileEntry[];
      dependencies: {
        minecraft: string;
        "fabric-loader"?: string;
        forge?: string;
        neoforge?: string;
        "quilt-loader"?: string;
      };
      /// 格式标记（用于区分 curseforge）
      format: "modrinth";
      /// 扩展字段（UI 用，不参与 mrpack 规范校验）
      loader?: string;
      loader_version?: string;
      author?: string;
      optifine?: boolean;
      optifine_version?: string | null;
      cross_loader?: boolean;
      created_at: number;
      updated_at: number;
    }
  | {
      format: "curseforge";
      name: string;
      version: string;
      author: string;
      created_at: number;
      updated_at: number;
      game_version: string;
      loader?: string;
      loader_version?: string;
      optifine?: boolean;
      optifine_version?: string | null;
      cross_loader?: boolean;
      files: CurseforgeFileEntry[];
    };

export async function fetchInstances(): Promise<ModpackListEntry[]> {
  try {
    const res = await invoke<ModpackListEntry[]>("list_modpack_instances");
    return res || [];
  } catch (e) {
    console.error("获取整合包列表失败:", e);
    return [];
  }
}

export async function saveInstance(
  data: ModpackInstance,
  minecraftPath?: string,
): Promise<void> {
  const payload: any = { ...data };
  await invoke("save_modpack_instance", {
    instance: payload,
    minecraftPath: minecraftPath || null,
  });
}

export async function loadInstance(
  name: string,
  minecraftPath?: string,
): Promise<ModpackInstance> {
  return await invoke<ModpackInstance>("load_modpack_instance", {
    name,
    minecraftPath: minecraftPath || null,
  });
}

export async function deleteInstance(
  name: string,
  minecraftPath?: string,
): Promise<void> {
  await invoke("delete_modpack_instance", {
    name,
    minecraftPath: minecraftPath || null,
  });
}

export async function renameInstance(
  oldName: string,
  newName: string,
  minecraftPath?: string,
): Promise<void> {
  await invoke("rename_modpack_instance", {
    oldName,
    newName,
    minecraftPath: minecraftPath || null,
  });
}

export async function getModpackDir(minecraftPath?: string): Promise<string> {
  try {
    return await invoke<string>("get_modpack_dir", {
      minecraftPath: minecraftPath || null,
    });
  } catch {
    return "";
  }
}

/**
 * 用于页面标题栏的时间格式化
 */
export function formatTimestamp(ts: number): string {
  if (!ts) return "";
  try {
    return new Date(ts * 1000).toLocaleString();
  } catch {
    return "";
  }
}

/** Hook：管理整合包列表 */
export function useModpackInstances() {
  const [instances, setInstances] = useState<ModpackListEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const reload = async () => {
    setLoading(true);
    const list = await fetchInstances();
    setInstances(list);
    setLoading(false);
  };

  useEffect(() => {
    reload();
  }, []);

  return { instances, loading, reload };
}

/** 将内部工程导出为标准 .mrpack 或 CurseForge .zip。 */
export async function exportInstance(
  name: string,
  outputPath: string,
  minecraftPath?: string,
): Promise<string> {
  return await invoke<string>("export_modpack_instance", {
    name,
    outputPath,
    minecraftPath: minecraftPath || null,
  });
}