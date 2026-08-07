"use client";

import React, { useState, useEffect } from "react";
import { motion } from "framer-motion";
import { Package, Folder, Loader2 } from "lucide-react";
import ResourcePanel from "@/components/resource-panel";
import { useInstancePath, getMcVersion, getModLoader } from "@/hooks/use-instance-path";
import { useResourceManager } from "@/hooks/use-resource-manager";
import { useLaunchContext } from "@/components/launch/launch-provider";
import { fadeSlideUp } from "@/lib/motion";
import { useI18n, type TranslationKey } from "@/components/i18n/use-i18n";

/**
 * 资源管理页面的配置
 *
 * 所有资源管理页面（模组 / 资源包 / 光影包 / 数据包 / 存档）都
 * 可以通过这个配置 + createResourcePage 工厂函数生成，避免
 * 90% 的样板代码重复。
 */
export interface ResourcePageConfig {
  /** 左列标题 */
  title: TranslationKey;
  /** 左列图标 */
  leftIcon: React.ReactNode;
  /** 左列图标背景色样式类（如 "bg-emerald-500/10"） */
  leftIconBg: string;
  /** 左列图标颜色样式类（如 "text-emerald-500"） */
  leftIconColor: string;
  /** 实例目录中的子文件夹名（如 "mods" / "resourcepacks" / "shaderpacks" / "datapacks" / "saves"） */
  instanceSubdir: string;
  /** 后端 cache kind 字符串（如 "mod" / "resourcepack" / "shaderpack" / "datapack" / "world"） */
  cacheKind: string;
  /** 是否需要传 modLoader（仅 mods 需要） */
  needsModLoader?: boolean;
  /**
   * 版本信息来源：
   * - "instance": 从 selectedInstance 读取（instance-settings 页面）
   * - "config": 从启动配置 config 读取（game-settings 页面，默认）
   */
  versionSource?: "instance" | "config";
  /** 允许通过的文件扩展名（小写，不含点） */
  extensions: string[];
  /** 左列是否支持进入子目录 */
  directoryNavigation?: boolean;
  /** 从文件名中去除扩展名的函数（用于简化显示） */
  simplifyName: (name: string) => string;
  /** 右列图标（默认：文件夹图标） */
  rightIcon?: React.ReactNode;
  /** 右列图标背景色 */
  rightIconBg?: string;
  /** 预览（查看器）回调 - 点击眼睛图标时触发（如投影预览） */
  onOpenViewer?: (file: { name: string; size: number; isDir?: boolean }, side: "left" | "right") => void;
  /** 启用预览按钮的文件扩展名（小写不含点） */
  viewerExtensions?: string[];
}

/**
 * 扩展的返回接口（供 mods 页面额外消费的 modInfo 数据）
 */
export interface ResourcePageExtra {
  instanceModInfo: Map<string, any>;
  cacheModInfo: Map<string, any>;
  mcVersion: string | undefined;
  modLoader: string | undefined;
  instanceDir: string | undefined;
  minecraftPath: string | undefined;
  versionName: string;
  instanceFiles: { name: string; size: number }[];
  cacheFiles: { name: string; size: number }[];
  addToInstance: (fileName: string) => Promise<void>;
  removeFromInstance: (fileName: string) => Promise<void>;
  deleteFromInstance: (fileName: string) => Promise<void>;
  deleteFromCache: (fileName: string) => Promise<void>;
  renameInInstance: (oldName: string, newName: string) => Promise<void>;
  refresh: () => void;
}

/**
 * 生成一个资源管理页面组件（基础版本，无详情页）。
 *
 * 用于资源包 / 光影包 / 数据包 / 存档等普通页面。
 */
export function createResourcePage(config: ResourcePageConfig): React.FC {
  const {
    title,
    leftIcon,
    leftIconBg,
    instanceSubdir,
    cacheKind,
    needsModLoader = false,
    versionSource = "config",
    extensions,
    directoryNavigation = false,
    simplifyName,
    rightIcon,
    rightIconBg = "bg-emerald-500/10",
    onOpenViewer,
    viewerExtensions,
  } = config;

  const Component: React.FC = () => {
    const { t } = useI18n();
    const { config: launcherConfig } = useLaunchContext();
    const { instanceDir, selectedInstance, minecraftPath, configLoaded, loading: instancesScanning } = useInstancePath();

    // 「准备中」中间状态：configLoaded=true 但 instancesScanning 或 其他依赖还没到
    const [preparingTimeout, setPreparingTimeout] = useState(false);
    useEffect(() => {
      if (!configLoaded) return;
      // 如果 1.2s 内依赖仍然缺失（表示真的未配置），才显示「未配置」页
      const t = window.setTimeout(() => setPreparingTimeout(true), 1200);
      return () => window.clearTimeout(t);
    }, [configLoaded]);

    // 根据 versionSource 决定版本信息的来源
    const versionName =
      versionSource === "instance"
        ? selectedInstance?.name || t("launch.noVersionSelected")
        : launcherConfig.versionName || t("launch.noVersionSelected");
    const mcVersion =
      versionSource === "instance"
        ? selectedInstance?.minecraft_version
        : getMcVersion(selectedInstance, launcherConfig.versionName);
    const modLoader = needsModLoader
      ? versionSource === "instance"
        ? selectedInstance?.loader
        : getModLoader(
            selectedInstance,
            launcherConfig.loadType,
            launcherConfig.loadName,
            launcherConfig.versionName,
          )
      : undefined;

    const {
      filteredInstanceFiles,
      filteredCacheFiles,
      instanceLoading,
      cacheLoading,
      instanceError,
      cacheError,
      addToInstance,
      removeFromInstance,
      deleteFromInstance,
      deleteFromCache,
      uploadFiles,
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
      instanceSubdir,
      cacheKind,
      mcVersion,
      modLoader,
      extensions,
      directoryNavigation,
    );

    if (!configLoaded || (configLoaded && !preparingTimeout && instancesScanning)) {
      return (
        <motion.div
          variants={fadeSlideUp}
          initial="initial"
          animate="animate"
          className="flex h-full flex-col items-center justify-center gap-3 text-center p-4"
        >
          <div className="size-12 rounded-full bg-muted flex items-center justify-center">
            <Loader2 className="size-6 text-muted-foreground animate-spin" />
          </div>
          <p className="text-sm font-medium">{t("pageFactory.loadingConfiguration")}</p>
          <p className="text-xs text-muted-foreground">{t("pageFactory.pleaseWait")}</p>
        </motion.div>
      );
    }

    if (!minecraftPath || (versionSource === "instance" && !instanceDir)) {
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
          <p className="text-sm font-medium">{t("pageFactory.gameDirectoryIsNotConfigured")}</p>
          <p className="text-xs text-muted-foreground">
            {versionSource === "instance"
              ? t("pageFactory.selectAnInstanceFirst")
              : t("pageFactory.configureTheMinecraftGameDirectoryOnTheLaunchPage")}
          </p>
        </motion.div>
      );
    }

    const description =
      versionSource === "instance"
        ? selectedInstance
          ? [
              selectedInstance.name,
              ...(modLoader ? [modLoader] : []),
              ...(mcVersion ? [`MC ${mcVersion}`] : []),
            ].join(" · ")
          : t("pageFactory.selectAnInstance")
        : [
            t("pageFactory.versionVersionName", { versionName: versionName }),
            ...(modLoader ? [modLoader] : []),
            ...(mcVersion && mcVersion !== versionName ? [t("pageFactory.vanillaMcVersion", { mcVersion: mcVersion })] : []),
          ].join(" · ");

    return (
      <ResourcePanel
        leftTitle={t(title)}
        leftDescription={description}
        leftIcon={leftIcon}
        leftIconBg={leftIconBg}
        leftFiles={filteredInstanceFiles}
        leftLoading={instanceLoading}
        leftError={instanceError}
        leftSearch={instanceSearch}
        setLeftSearch={setInstanceSearch}
        leftBadge={t("pageFactory.length", { length: instanceFiles.length })}
        leftDirectoryPath={directoryNavigation ? instanceDirectoryPath : undefined}
        onOpenLeftDirectory={directoryNavigation ? openInstanceDirectory : undefined}
        onNavigateUpLeft={directoryNavigation ? goToParentInstanceDirectory : undefined}
        rightTitle=""
        rightIcon={rightIcon || <Folder className="size-5 text-emerald-500" />}
        rightIconBg={rightIconBg}
        rightFiles={filteredCacheFiles}
        rightLoading={cacheLoading}
        rightError={cacheError}
        rightSearch={cacheSearch}
        setRightSearch={setCacheSearch}
        rightBadge={t("pageFactory.length", { length: cacheFiles.length })}
        onMoveRightToLeft={addToInstance}
        onMoveLeftToRight={removeFromInstance}
        onDeleteLeft={deleteFromInstance}
        onDeleteRight={deleteFromCache}
        onRefresh={refresh}
        onUploadFiles={uploadFiles}
        simplifyName={simplifyName}
        onOpenViewer={onOpenViewer}
        viewerExtensions={viewerExtensions}
      />
    );
  };

  Component.displayName = `ResourcePage(${config.cacheKind})`;
  return Component;
}

/**
 * 用工厂方式调用 useResourceManager（供 mods 页面复用，它有额外的详情页）。
 * 返回完整的 hook 数据 + 页面配置信息，页面组件自行处理渲染分支。
 */
export function useResourcePage(config: ResourcePageConfig): {
  panel: Omit<
    React.ComponentProps<typeof ResourcePanel>,
    "leftTitle" | "leftIcon" | "leftIconBg" | "rightIcon" | "rightIconBg"
  > & {
    leftTitle: string;
    leftIcon: React.ReactNode;
    leftIconBg: string;
    leftDescription: string;
    rightIcon: React.ReactNode;
    rightIconBg: string;
    rightTitle: string;
  };
  loadingState: { configLoaded: boolean; minecraftPath: string | undefined; instanceDir: string | undefined; instancesScanning: boolean };
  extra: ResourcePageExtra;
} {
  const { t } = useI18n();
  const {
    title,
    leftIcon,
    leftIconBg,
    instanceSubdir,
    cacheKind,
    needsModLoader = false,
    versionSource = "config",
    extensions,
    directoryNavigation = false,
    simplifyName,
    rightIcon,
    rightIconBg = "bg-emerald-500/10",
    onOpenViewer,
    viewerExtensions,
  } = config;

  const { config: launcherConfig } = useLaunchContext();
  const { instanceDir, selectedInstance, minecraftPath, configLoaded, loading: instancesScanning } = useInstancePath();

  const versionName =
    versionSource === "instance"
      ? selectedInstance?.name || t("launch.noVersionSelected")
      : launcherConfig.versionName || t("launch.noVersionSelected");
  const mcVersion =
    versionSource === "instance"
      ? selectedInstance?.minecraft_version
      : getMcVersion(selectedInstance, launcherConfig.versionName);
  const modLoader = needsModLoader
    ? versionSource === "instance"
      ? selectedInstance?.loader
      : getModLoader(
          selectedInstance,
          launcherConfig.loadType,
          launcherConfig.loadName,
          launcherConfig.versionName,
        )
    : undefined;

  const manager = useResourceManager(
    instanceDir,
    instanceSubdir,
    cacheKind,
    mcVersion,
    modLoader,
    extensions,
    directoryNavigation,
  );

  const description =
    versionSource === "instance"
      ? selectedInstance
        ? [
            selectedInstance.name,
            ...(modLoader ? [modLoader] : []),
            ...(mcVersion ? [`MC ${mcVersion}`] : []),
          ].join(" · ")
        : t("pageFactory.selectAnInstance")
      : [
          t("pageFactory.versionVersionName", { versionName: versionName }),
          ...(modLoader ? [modLoader] : []),
          ...(mcVersion && mcVersion !== versionName ? [t("pageFactory.vanillaMcVersion", { mcVersion: mcVersion })] : []),
        ].join(" · ");

  return {
    panel: {
      leftTitle: t(title),
      leftDescription: description,
      leftIcon,
      leftIconBg,
      leftFiles: manager.filteredInstanceFiles,
      leftLoading: manager.instanceLoading,
      leftError: manager.instanceError,
      leftSearch: manager.instanceSearch,
      setLeftSearch: manager.setInstanceSearch,
      leftBadge: t("pageFactory.length", { length: manager.instanceFiles.length }),
      leftDirectoryPath: directoryNavigation ? manager.instanceDirectoryPath : undefined,
      onOpenLeftDirectory: directoryNavigation ? manager.openInstanceDirectory : undefined,
      onNavigateUpLeft: directoryNavigation ? manager.goToParentInstanceDirectory : undefined,
      leftModInfo: manager.instanceModInfo,
      rightTitle: "",
      rightIcon: rightIcon || <Folder className="size-5 text-emerald-500" />,
      rightIconBg,
      rightFiles: manager.filteredCacheFiles,
      rightLoading: manager.cacheLoading,
      rightError: manager.cacheError,
      rightSearch: manager.cacheSearch,
      setRightSearch: manager.setCacheSearch,
      rightBadge: t("pageFactory.length", { length: manager.cacheFiles.length }),
      rightModInfo: manager.cacheModInfo,
      onMoveRightToLeft: manager.addToInstance,
      onMoveLeftToRight: manager.removeFromInstance,
      onDeleteLeft: manager.deleteFromInstance,
      onDeleteRight: manager.deleteFromCache,
      onRenameLeft: manager.renameInInstance,
      onRefresh: manager.refresh,
      onUploadFiles: manager.uploadFiles,
      simplifyName,
      onOpenViewer,
      viewerExtensions,
    },
    loadingState: { configLoaded, minecraftPath, instanceDir, instancesScanning },
    extra: {
      instanceModInfo: manager.instanceModInfo,
      cacheModInfo: manager.cacheModInfo,
      mcVersion,
      modLoader,
      instanceDir,
      minecraftPath,
      versionName,
      instanceFiles: manager.instanceFiles,
      cacheFiles: manager.cacheFiles,
      addToInstance: manager.addToInstance,
      removeFromInstance: manager.removeFromInstance,
      deleteFromInstance: manager.deleteFromInstance,
      deleteFromCache: manager.deleteFromCache,
      renameInInstance: manager.renameInInstance,
      refresh: manager.refresh,
    },
  };
}

/**
 * 通用的加载/未配置 fallback 页面
 */
export function ResourcePageFallback({
  title,
  subtitle,
}: {
  title: TranslationKey;
  subtitle: TranslationKey;
}) {
  const { t } = useI18n();
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
      <p className="text-sm font-medium">{t(title)}</p>
      <p className="text-xs text-muted-foreground">{t(subtitle)}</p>
    </motion.div>
  );
}