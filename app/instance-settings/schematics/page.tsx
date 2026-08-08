"use client";

import React, { useCallback } from "react";
import { motion } from "framer-motion";
import { Boxes, Folder, Package } from "lucide-react";
import { useRouter } from "next/navigation";
import ResourcePanel from "@/components/resource-panel";
import { useInstancePath } from "@/hooks/use-instance-path";
import { useResourceManager } from "@/hooks/use-resource-manager";
import { fadeSlideUp } from "@/lib/motion";
import { invoke } from "@tauri-apps/api/core";
import { FileItem } from "@/components/resource-panel";

export default function SchematicsPage() {
  const router = useRouter();
  const { instanceDir, selectedInstance, minecraftPath, configLoaded } = useInstancePath();

  const viewerExtensions = ["schem", "schematic", "litematic", "nbt"];

  const handleOpenViewer = useCallback(async (file: FileItem, side: "left" | "right") => {
    if (!file) return;
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
            mcVersion: selectedInstance?.minecraft_version ?? "",
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
  }, [instanceDir, router, selectedInstance]);

  const {
    filteredInstanceFiles,
    filteredCacheFiles,
    instanceLoading,
    cacheLoading,
    instanceError,
    cacheError,
    addToInstance,
    removeFromInstance,
    refresh,
    instanceSearch,
    setInstanceSearch,
    cacheSearch,
    setCacheSearch,
    instanceFiles,
    cacheFiles,
    openInstanceDirectory,
    goToParentInstanceDirectory,
    instanceDirectoryPath,
  } = useResourceManager(
    instanceDir,
    "schematics",
    "world",
    selectedInstance?.minecraft_version,
    undefined,
    ["schem", "schematic", "litematic", "nbt"],
    true,
  );

  if (!configLoaded) {
    return (
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        className="flex h-full flex-col items-center justify-center gap-3 text-center p-4"
      >
        <div className="size-12 rounded-full bg-muted flex items-center justify-center">
          <Package className="size-6 text-muted-foreground" />
        </div>
        <p className="text-sm font-medium">正在加载配置...</p>
        <p className="text-xs text-muted-foreground">请稍候</p>
      </motion.div>
    );
  }

  if (!minecraftPath || !instanceDir) {
    return (
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        className="flex h-full flex-col items-center justify-center gap-3 text-center p-4"
      >
        <div className="size-12 rounded-full bg-muted flex items-center justify-center">
          <Package className="size-6 text-muted-foreground" />
        </div>
        <p className="text-sm font-medium">未配置游戏目录</p>
        <p className="text-xs text-muted-foreground">请先在「启动」页面配置游戏目录路径</p>
      </motion.div>
    );
  }

  return (
    <ResourcePanel
      leftTitle="Schematics 管理"
      leftDescription={
        selectedInstance
          ? `${selectedInstance.name} · ${instanceFiles.length} 个文件`
          : "请选择一个实例"
      }
      leftIcon={<Boxes className="size-5 text-rose-500" />}
      leftIconBg="bg-rose-500/10"
      leftFiles={filteredInstanceFiles}
      leftLoading={instanceLoading}
      leftError={instanceError}
      leftSearch={instanceSearch}
      setLeftSearch={setInstanceSearch}
      leftBadge={`${instanceFiles.length} 个`}
      leftDirectoryPath={instanceDirectoryPath}
      onOpenLeftDirectory={openInstanceDirectory}
      onNavigateUpLeft={goToParentInstanceDirectory}
      rightTitle=""
      rightIcon={<Folder className="size-5 text-sky-500" />}
      rightIconBg="bg-sky-500/10"
      rightFiles={filteredCacheFiles}
      rightLoading={cacheLoading}
      rightError={cacheError}
      rightSearch={cacheSearch}
      setRightSearch={setCacheSearch}
      rightBadge={`${cacheFiles.length} 个`}
      onMoveRightToLeft={addToInstance}
      onMoveLeftToRight={removeFromInstance}
      onRefresh={refresh}
      onOpenViewer={handleOpenViewer}
      viewerExtensions={viewerExtensions}
    />
  );
}