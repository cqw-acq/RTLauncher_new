"use client";

import React, { useCallback } from "react";
import { Box } from "lucide-react";
import { createResourcePage } from "@/components/resource-page-factory";
import { useRouter } from "next/navigation";
import { useInstancePath } from "@/hooks/use-instance-path";
import { invoke } from "@tauri-apps/api/core";

const SCHEMATIC_EXTS = ["schem", "schematic", "litematic", "nbt"];

export default function SchematicsPageWrapper() {
  const router = useRouter();
  const { instanceDir, configLoaded } = useInstancePath();

  const handleOpenViewer = useCallback(async (file: { name: string; size: number; isDir?: boolean; path?: string }, side: "left" | "right") => {
    if (!configLoaded || file.isDir) return;
    try {
      let absPath: string | null = null;
      if (side === "left" && instanceDir) {
        absPath = `${instanceDir}/schematics/${file.name}`;
      } else if (side === "right") {
        if (typeof (file as any).path === "string") {
          absPath = (file as any).path;
        } else if (file.name) {
          const cacheRoot = await invoke<string | null>("get_mod_cache_dir_cmd", {
            kind: "world",
            mcVersion: "",
            modLoader: "",
          }).catch(() => null);
          if (cacheRoot) {
            absPath = `${cacheRoot.replace(/\\/g, "/")}/${file.name}`;
          }
        }
      }
      if (!absPath) return;

      const b64 = await invoke<string>("read_file_base64", { path: absPath });
      const key = `schematic_viewer:${Date.now()}`;
      sessionStorage.setItem(key, JSON.stringify({ name: file.name, b64 }));
      router.push(`/game-settings/schematics/viewer?k=${encodeURIComponent(key)}`);
    } catch (e) {
      console.error("预览打开失败", e);
    }
  }, [instanceDir, configLoaded, router]);

  // 通过 createResourcePage 返回的组件内部无法直接传 viewer props，
  // 这里改为渲染组件时再包一层注入。实际上 createResourcePage 已经支持
  // 传入 onOpenViewer/viewerExtensions，只是上面调用时没有传。
  // 重新用带 viewer 的工厂生成：
  const ViewerResourcePage = createResourcePage({
    title: "panel.schematics",
    leftIcon: <Box className="size-5 text-rose-500" />,
    leftIconBg: "bg-rose-500/10",
    leftIconColor: "text-rose-500",
    instanceSubdir: "schematics",
    cacheKind: "world",
    extensions: SCHEMATIC_EXTS,
    directoryNavigation: true,
    simplifyName: (name) => name.replace(/\.(schem|schematic|litematic|nbt)$/i, ""),
    onOpenViewer: handleOpenViewer,
    viewerExtensions: SCHEMATIC_EXTS,
  });

  return <ViewerResourcePage />;
}