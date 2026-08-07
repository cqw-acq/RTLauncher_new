"use client";

import React, { useState } from "react";
import { motion } from "framer-motion";
import {
  Search,
  ArrowRight,
  RefreshCw,
  Plus,
  Trash2,
  Edit3,
  Check,
  X,
  Info,
  Folder,
  ChevronLeft,
  Eye,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { fadeSlideUp, staggerContainer, staggerItem } from "@/lib/motion";
import { useI18n } from "@/components/i18n/use-i18n";

/**
 * 模组依赖项信息
 */
export interface ModDependency {
  mod_id: string;
  version_range?: string | null;
  mandatory: boolean;
  ordering?: string | null;
  side?: string | null;
}

/**
 * 模组完整元数据信息
 */
export interface ModInfo {
  file_name: string;
  mod_id: string;
  name: string;
  version: string;
  description?: string | null;
  authors: string[];
  license?: string | null;
  icon?: string | null;
  source?: string | null;
  homepage?: string | null;
  issues?: string | null;
  minecraft_version?: string | null;
  mod_loader?: string | null;
  dependencies: ModDependency[];
  optional_dependencies: ModDependency[];
  incompatible_dependencies: ModDependency[];
}

/**
 * 通用的两列资源管理页面布局
 *
 * 左列：已加入实例的文件
 * 右列：cache 中对应版本的文件（可加入）
 */

export interface FileItem {
  name: string;
  size: number;
  isDir?: boolean;
}

export interface ResourcePanelProps {
  // 左列
  leftTitle: string;
  leftDescription?: string;
  leftIcon: React.ReactNode;
  leftIconBg: string;
  leftFiles: FileItem[];
  leftLoading: boolean;
  leftError: string | null;
  leftSearch: string;
  setLeftSearch: (s: string) => void;
  leftBadge?: string;
  /** 当前实例目录下的相对路径；传入后显示面包屑 */
  leftDirectoryPath?: string[];
  /** 双击左列目录时调用 */
  onOpenLeftDirectory?: (directoryName: string) => void;
  /** 返回左列的上一级目录 */
  onNavigateUpLeft?: () => void;
  // 左列模组元数据缓存
  leftModInfo?: Map<string, ModInfo>;
  // 右列
  rightTitle: string;
  rightDescription?: string;
  rightIcon: React.ReactNode;
  rightIconBg: string;
  rightFiles: FileItem[];
  rightLoading: boolean;
  rightError: string | null;
  rightSearch: string;
  setRightSearch: (s: string) => void;
  rightBadge?: string;
  // 右列模组元数据缓存
  rightModInfo?: Map<string, ModInfo>;
  // 模组详情页回调（点击文件名时调用）
  onOpenModDetail?: (fileName: string, info: ModInfo) => void;
  // 操作
  onMoveRightToLeft?: (fileName: string) => Promise<void>; // 加入实例（cache -> 实例）
  onMoveLeftToRight?: (fileName: string) => Promise<void>; // 移出实例（实例 -> cache）
  onDeleteLeft?: (fileName: string) => Promise<void>; // 从实例中删除
  onDeleteRight?: (fileName: string) => Promise<void>; // 从 cache 中删除
  onRenameLeft?: (oldName: string, newName: string) => Promise<void>; // 重命名实例中的文件
  onRefresh?: () => void;
  // 文件上传回调
  onUploadFiles?: () => Promise<void>;
  // 文件名简化（可选回调）
  simplifyName?: (name: string) => string;
  // 文件副标题（可选回调）- 从文件名或其他信息生成的描述
  getFileSubtitle?: (file: FileItem) => string;
  // 预览（查看器）回调 - 点击眼睛图标时触发（如投影预览）
  onOpenViewer?: (file: FileItem, side: "left" | "right") => void;
  // 启用预览按钮的文件扩展名（小写不含点，如 ["litematic", "schem", "schematic", "nbt"]）
  viewerExtensions?: string[];
}

export default function ResourcePanel({
  leftTitle,
  leftDescription,
  leftIcon,
  leftIconBg,
  leftFiles,
  leftLoading,
  leftError,
  leftSearch,
  setLeftSearch,
  leftBadge,
  leftDirectoryPath = [],
  onOpenLeftDirectory,
  onNavigateUpLeft,
  leftModInfo,
  rightIcon,
  rightIconBg,
  rightFiles,
  rightLoading,
  rightError,
  rightSearch,
  setRightSearch,
  rightBadge,
  rightModInfo,
  onOpenModDetail,
  onMoveRightToLeft,
  onMoveLeftToRight,
  onDeleteLeft,
  onDeleteRight,
  onRenameLeft,
  onRefresh,
  onUploadFiles,
  simplifyName,
  getFileSubtitle,
  onOpenViewer,
  viewerExtensions,
}: ResourcePanelProps) {
  const { t } = useI18n();
  // 先做 URL decode（%20 -> 空格 等），再应用用户自定义的 simplifyName
  const decodeUrlName = (s: string): string => {
    try {
      return decodeURIComponent(s.replace(/\+/g, " "));
    } catch {
      return s;
    }
  };
  const simplify = simplifyName
    ? (n: string) => simplifyName(decodeUrlName(n))
    : (n: string) => decodeUrlName(n);
  const [renamingName, setRenamingName] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  const formatSize = (bytes: number): string => {
    if (bytes === 0) return "";
    const units = ["B", "KB", "MB", "GB"];
    let size = bytes;
    let unitIndex = 0;
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    return `${size.toFixed(1)} ${units[unitIndex]}`;
  };

  const handleStartRename = (name: string) => {
    setRenamingName(name);
    setRenameValue(name);
  };

  const handleCancelRename = () => {
    setRenamingName(null);
    setRenameValue("");
  };

  const handleConfirmRename = async () => {
    if (!renamingName || !onRenameLeft) return;
    if (!renameValue.trim()) return;
    try {
      await onRenameLeft(renamingName, renameValue.trim());
    } catch (e) {
      console.error("重命名失败:", e);
    }
    setRenamingName(null);
    setRenameValue("");
  };

  const handleOpenDetail = (file: FileItem, infoMap?: Map<string, ModInfo>) => {
    if (!onOpenModDetail) return;
    if (infoMap && infoMap.size > 0) {
      const info = infoMap.get(file.name);
      if (info) {
        onOpenModDetail(file.name, info);
        return;
      }
    }
    // 如果没有模组信息或Map为空，仍然调用回调（用于存档等非模组页面）
    onOpenModDetail(file.name, {} as ModInfo);
  };

  const renderFileList = (
    files: FileItem[],
    loading: boolean,
    error: string | null,
    emptyIcon: React.ReactNode,
    emptyText: string,
    side: "left" | "right", // 左列(实例)或右列(cache)
    moveHandler: ((fileName: string) => Promise<void>) | undefined,
    deleteHandler: ((fileName: string) => Promise<void>) | undefined,
    canRename: boolean,
    modInfoMap?: Map<string, ModInfo>,
  ) => {
    const canViewer = !!onOpenViewer;
    const matchViewer = (name: string) => {
      if (!viewerExtensions || viewerExtensions.length === 0) return true;
      const dot = name.lastIndexOf(".");
      if (dot < 0) return false;
      const ext = name.slice(dot + 1).toLowerCase();
      return viewerExtensions.includes(ext);
    };
    if (loading) {
      return (
        <div className="space-y-2 px-1">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="h-14 rounded-lg bg-muted/50 animate-pulse" />
          ))}
        </div>
      );
    }
    if (error) {
      return (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center gap-2 p-6 text-center"
        >
          <p className="text-sm text-destructive">{t("panel.failedToLoad")}</p>
          <p className="text-xs text-muted-foreground">{error}</p>
        </motion.div>
      );
    }
    if (files.length === 0) {
      return (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex flex-col items-center justify-center gap-3 p-6 text-center"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">{emptyIcon}</div>
          <p className="text-sm font-medium">{emptyText}</p>
        </motion.div>
      );
    }
    return (
      <motion.div variants={staggerContainer} initial="initial" animate="animate" className="space-y-1">
        {files.map((file) => {
          const info = modInfoMap?.get(file.name);
          const displayName = info?.name && info.name !== file.name
            ? info.name // 优先显示模组名称
            : simplify(file.name);
          const subtitle = info
            ? [
                `v${info.version}`,
                info.mod_id,
                info.authors.length > 0 ? info.authors.slice(0, 2).join(", ") : null,
              ].filter(Boolean).join(" · ")
            : (getFileSubtitle ? getFileSubtitle(file) : formatSize(file.size));
          const hasDetail = !!onOpenModDetail;
          const canOpenDirectory = file.isDir && !!onOpenLeftDirectory;

          return (
            <motion.div
              key={file.name}
              variants={staggerItem}
              className={`flex items-center gap-3 px-3 py-2.5 rounded-lg hover:bg-accent/50 transition-colors group ${
                canOpenDirectory ? "cursor-pointer" : ""
              }`}
              role={canOpenDirectory ? "button" : undefined}
              tabIndex={canOpenDirectory ? 0 : undefined}
              aria-label={canOpenDirectory ? t("resource.openFolder", { name: file.name }) : undefined}
              onDoubleClick={canOpenDirectory ? (event) => {
                if ((event.target as HTMLElement).closest("button")) return;
                onOpenLeftDirectory(file.name);
              } : undefined}
              onKeyDown={canOpenDirectory ? (event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onOpenLeftDirectory(file.name);
                }
              } : undefined}
            >
              {file.isDir && (
                <Folder className="size-4 shrink-0 text-amber-500" aria-hidden="true" />
              )}
              <div className="flex-1 min-w-0">
                {canRename && renamingName === file.name ? (
                  <div className="flex items-center gap-2">
                    <Input
                      value={renameValue}
                      onChange={(e) => setRenameValue(e.target.value)}
                      className="h-7 text-xs"
                      autoFocus
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleConfirmRename();
                        if (e.key === "Escape") handleCancelRename();
                      }}
                    />
                    <Button variant="ghost" size="icon" className="size-7" onClick={handleConfirmRename}>
                      <Check className="size-3.5" />
                    </Button>
                    <Button variant="ghost" size="icon" className="size-7" onClick={handleCancelRename}>
                      <X className="size-3.5" />
                    </Button>
                  </div>
                ) : (
                  <>
                    <p
                      className="text-sm font-bold truncate"
                      title={file.name}
                    >
                      {displayName}
                    </p>
                    {subtitle && (
                      <p className="text-xs text-muted-foreground truncate">{subtitle}</p>
                    )}
                  </>
                )}
              </div>

              {/* 操作按钮组 - 只有在不是重命名模式时才显示 */}
              {!(canRename && renamingName === file.name) && (
                <div className="flex items-center gap-1 shrink-0">
                  {/* 查看详情（模组专用）- "!" 字符按钮 */}
                  {hasDetail && (
                    <Button
                      variant="secondary"
                      size="icon"
                      className="size-7 shrink-0"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleOpenDetail(file, modInfoMap);
                      }}
                      title={t("panel.viewDetails")}
                    >
                      <Info className="size-3.5" />
                    </Button>
                  )}

                  {/* 预览（查看器，如投影预览）- 眼睛图标 */}
                  {canViewer && !file.isDir && matchViewer(file.name) && (
                    <Button
                      variant="secondary"
                      size="icon"
                      className="size-7 shrink-0"
                      onClick={(e) => {
                        e.stopPropagation();
                        onOpenViewer!(file, side);
                      }}
                      title="预览"
                    >
                      <Eye className="size-3.5" />
                    </Button>
                  )}

                  {/* 重命名（仅左列） */}
                  {canRename && onRenameLeft && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7 opacity-0 group-hover:opacity-100 transition-opacity"
                      onClick={() => handleStartRename(file.name)}
                      title={t("panel.rename")}
                    >
                      <Edit3 className="size-3.5" />
                    </Button>
                  )}

                  {/* 移动按钮 - 仅图标，无文字，加粗效果 */}
                  {moveHandler && (
                    <Button
                      variant={side === "right" ? "default" : "secondary"}
                      size="icon"
                      className={`size-7 ${side === "right" ? "font-bold" : ""}`}
                      onClick={() => moveHandler(file.name)}
                      title={side === "right" ? t("resource.addToInstance") : t("resource.removeFromInstance")}
                    >
                      {side === "right" ? (
                        <Plus className="size-3.5" />
                      ) : (
                        <ArrowRight className="size-3.5" />
                      )}
                    </Button>
                  )}

                  {/* 删除按钮 */}
                  {deleteHandler && (
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7 text-destructive opacity-0 group-hover:opacity-100 transition-opacity hover:bg-destructive/10"
                      onClick={() => {
                        if (confirm(t("resource.deleteConfirm", { name: simplify(file.name) }))) {
                          deleteHandler(file.name);
                        }
                      }}
                      title={t("common.delete")}
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  )}
                </div>
              )}
            </motion.div>
          );
        })}
      </motion.div>
    );
  };

  const leftEmptyIcon = React.cloneElement(leftIcon as React.ReactElement<{ className?: string }>, { className: "size-6 text-muted-foreground" });
  const rightEmptyIcon = React.cloneElement(rightIcon as React.ReactElement<{ className?: string }>, { className: "size-6 text-muted-foreground" });

  return (
    <div className="flex h-full flex-col gap-4 p-4 overflow-hidden">
      {/* 标题栏 */}
      <motion.div
        variants={fadeSlideUp}
        initial="initial"
        animate="animate"
        className="flex items-center gap-3 shrink-0"
      >
        <div className={`flex size-9 items-center justify-center rounded-xl ${leftIconBg}`}>{leftIcon}</div>
        <div>
          <h1 className="text-lg font-semibold leading-none">{leftTitle}</h1>
          <p className="mt-1 text-xs text-muted-foreground">{leftDescription || ""}</p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {onUploadFiles && (
            <Button variant="default" size="icon" className="size-8" onClick={onUploadFiles} title={t("resource.uploadFiles")}>
              <Plus className="size-3.5" />
            </Button>
          )}
          {onRefresh && (
            <Button variant="ghost" size="icon" className="size-8" onClick={onRefresh} title={t("common.refresh")}>
              <RefreshCw className="size-3.5" />
            </Button>
          )}
        </div>
      </motion.div>

      {/* 两列内容区 */}
      <div className="flex-1 flex gap-4 min-h-0 overflow-hidden">
        {/* 左列：实例中的文件 */}
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex-1 flex flex-col border rounded-xl bg-card overflow-hidden min-w-0"
        >
          <div className="shrink-0 flex items-center gap-3 px-3 py-2 border-b">
            <div className={`flex size-7 items-center justify-center rounded-lg ${leftIconBg}`}>
              {React.cloneElement(leftIcon as React.ReactElement<{ className?: string }>, { className: "size-3.5 text-current" })}
            </div>
                <div className="min-w-0 flex-1">
                  <h2 className="text-sm font-semibold leading-tight">{t("resource.currentInstance")}</h2>
                  {leftDirectoryPath.length > 0 ? (
                    <div className="flex items-center gap-1 text-xs text-muted-foreground min-w-0">
                      {onNavigateUpLeft && (
                        <button
                          type="button"
                          className="shrink-0 hover:text-foreground"
                          onClick={onNavigateUpLeft}
                          title={t("panel.goToParentFolder")}
                        >
                          <ChevronLeft className="inline size-3.5" /> {t("common.back")}
                        </button>
                      )}
                      <span className="truncate" title={leftDirectoryPath.join(" / ")}>
                        {leftDirectoryPath.join(" / ")}
                      </span>
                    </div>
                  ) : (
                    <p className="text-xs text-muted-foreground truncate">
                      {onOpenLeftDirectory ? t("resource.doubleClickFolder") : t("resource.addedFiles")}
                    </p>
                  )}
                </div>
            {leftBadge && <Badge variant="secondary" className="text-xs shrink-0">{leftBadge}</Badge>}
            <div className="relative w-40 shrink-0">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
              <Input
                value={leftSearch}
                onChange={(e) => setLeftSearch(e.target.value)}
                placeholder={t("common.search")}
                className="pl-7 h-7 text-xs"
              />
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-3">
            {renderFileList(
              leftFiles,
              leftLoading,
              leftError,
              leftEmptyIcon,
              t("resource.noFiles"),
              "left",
              onMoveLeftToRight,
              onDeleteLeft,
              true,
              leftModInfo,
            )}
          </div>
        </motion.div>

        {/* 右列：cache 中的文件 */}
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex-1 flex flex-col border rounded-xl bg-card overflow-hidden min-w-0"
        >
          <div className="shrink-0 flex items-center gap-3 px-3 py-2 border-b">
            <div className={`flex size-7 items-center justify-center rounded-lg ${rightIconBg}`}>
              {React.cloneElement(rightIcon as React.ReactElement<{ className?: string }>, { className: "size-3.5 text-current" })}
            </div>
            <div className="min-w-0 flex-1">
              <h2 className="text-sm font-semibold leading-tight">{t("resource.cacheLibrary")}</h2>
              <p className="text-xs text-muted-foreground truncate">{t("resource.matchingVersionAvailable")}</p>
            </div>
            {rightBadge && <Badge variant="secondary" className="text-xs shrink-0">{rightBadge}</Badge>}
            <div className="relative w-40 shrink-0">
              <Search className="absolute left-2 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
              <Input
                value={rightSearch}
                onChange={(e) => setRightSearch(e.target.value)}
                placeholder={t("common.search")}
                className="pl-7 h-7 text-xs"
              />
            </div>
          </div>
          <div className="flex-1 overflow-y-auto p-3">
            {renderFileList(
              rightFiles,
              rightLoading,
              rightError,
              rightEmptyIcon,
              t("resource.noAvailableFiles"),
              "right",
              onMoveRightToLeft,
              onDeleteRight,
              false,
              rightModInfo,
            )}
          </div>
        </motion.div>
      </div>
    </div>
  );
}