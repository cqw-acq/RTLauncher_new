import {
  Camera,
  Database,
  Download,
  Gamepad2,
  Globe,
  Map,
  Palette,
  Puzzle,
  Sparkles,
  type LucideIcon,
} from "lucide-react";

import type { HomeQuickAction } from "@/types";
import type { TranslationKey } from "@/components/i18n/use-i18n";

/**
 * 动画过渡时长常量 (ms)
 */
export const TRANSITION_DURATION = {
  /** 页面切换过渡时长 */
  PAGE_TRANSITION: 700,
  /** 主题切换动画时长 */
  THEME_TOGGLE: 300,
  /** 通用组件过渡时长 */
  DEFAULT: 200,
} as const;

/**
 * 布局尺寸常量
 */
export const LAYOUT = {
  /** 侧边栏宽度 (px) */
  SIDEBAR_WIDTH: 56,
  /** 主页面左右卡片宽度比例 */
  CARD_WIDTH_RATIO: "1/4",
  /** 最小触摸区域尺寸 (px) */
  MIN_TOUCH_TARGET: 44,
} as const;

/**
 * 路由配置
 */
export const ROUTES = {
  HOME: "/",
  LAUNCH: "/launch",
  DOWNLOAD: "/download",
  TOOLS: "/tools",
  SETTINGS: "/settings",
  INSTANCE_SETTINGS: {
    ROOT: "/instance-settings",
    MODS: "/instance-settings/mods",
    WORLDS: "/instance-settings/worlds",
    RESOURCES: "/instance-settings/resources",
    SHADERS: "/instance-settings/shaders",
    SCREENSHOTS: "/instance-settings/screenshots",
    SCHEMATICS: "/instance-settings/schematics",
  },
} as const;

/**
 * 本地存储键名
 */
export const STORAGE_KEYS = {
  /** 当前视图状态 */
  CURRENT_VIEW: "rtl-currentView",
  /** 主题设置 */
  THEME: "theme",
} as const;

/**
 * Z-index 层级
 */
export const Z_INDEX = {
  SIDEBAR: 10,
  HEADER: 10,
  DIALOG: 50,
  TOAST: 100,
} as const;

/**
 * 首页快捷入口配置（数据驱动，新增入口只需在此追加）
 */
const homeQuickAction = (
  href: string,
  titleKey: TranslationKey,
  descriptionKey: TranslationKey,
  icon: LucideIcon,
  iconClassName: string,
): HomeQuickAction => ({ href, titleKey, descriptionKey, icon, iconClassName });

/** 首页上方通用功能入口 */
export const HOME_QUICK_ACTIONS: HomeQuickAction[] = [
  homeQuickAction("/download", "home.downloads", "home.gameLoadersAndResources", Download, "bg-sky-500/10 text-sky-600 dark:text-sky-400"),
  homeQuickAction("/game-settings", "home.gameManagement", "home.modsWorldsAndResourcePacks", Gamepad2, "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"),
];

/** 首页游戏资源快捷入口 */
export const HOME_GAME_QUICK_ACTIONS: HomeQuickAction[] = [
  homeQuickAction("/game-settings/mods", "home.cardGrid.mods", "home.cardGrid.manageYourMods", Puzzle, "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"),
  homeQuickAction("/game-settings/screenshots", "home.cardGrid.screenshots", "home.cardGrid.manageGameScreenshots", Camera, "bg-cyan-500/10 text-cyan-600 dark:text-cyan-400"),
  homeQuickAction("/game-settings/schematics", "home.cardGrid.schematics", "home.cardGrid.manageBuildingDesigns", Map, "bg-teal-500/10 text-teal-600 dark:text-teal-400"),
  homeQuickAction("/game-settings/resources", "home.cardGrid.resourcePacks", "home.cardGrid.manageGameTextures", Palette, "bg-blue-500/10 text-blue-600 dark:text-blue-400"),
  homeQuickAction("/game-settings/datapacks", "gameSettings.datapacks", "gameSettings.manageMinecraftDatapacks", Database, "bg-fuchsia-500/10 text-fuchsia-600 dark:text-fuchsia-400"),
  homeQuickAction("/game-settings/shaders", "home.cardGrid.shaders", "home.cardGrid.enhancedVisualEffects", Sparkles, "bg-violet-500/10 text-violet-600 dark:text-violet-400"),
  homeQuickAction("/game-settings/worlds", "home.cardGrid.worlds", "home.cardGrid.manageWorldSaves", Globe, "bg-amber-500/10 text-amber-600 dark:text-amber-400"),
];
