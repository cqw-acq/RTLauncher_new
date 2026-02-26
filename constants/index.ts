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
